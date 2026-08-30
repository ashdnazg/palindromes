use ethnum::u256;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub trait Bits {
    fn bits(&self) -> u32;
}

impl Bits for u256 {
    fn bits(&self) -> u32 {
        Self::BITS - self.leading_zeros()
    }
}

fn default_backward_depth(dec_length: u32) -> u32 {
    (dec_length as f64 * 5f64.log2() / (2f64 * 5f64.log2() + 1f64) / 2f64).floor() as u32
}

fn get_max_cache(length: u32, base: u32) -> Vec<u256> {
    let cache_length = length.div_ceil(2);
    (1..cache_length)
        .map(|i| u256::from(base).pow(length - i) - u256::from(base).pow(i))
        .collect()
}

fn get_digit_cache(dec_length: u32) -> Vec<[u256; 10]> {
    let cache_length = dec_length.div_ceil(2);
    (0..cache_length)
        .map(|i| {
            let j = dec_length - i - 1;
            let mut entry = u256::from(10u32).pow(i);
            if i != j {
                entry += u256::from(10u32).pow(j);
            }

            std::array::from_fn(|i| entry * i as u128)
        })
        .collect()
}

#[inline]
fn mask_lo(bits: u32) -> u64 {
    if bits >= 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    }
}

#[inline]
fn top_64_bits(num: u256, bin_length: u32) -> u64 {
    match bin_length {
        0..=64 => num.as_u64() << (64 - bin_length),
        65..=128 => ((num.low() << (128 - bin_length)) >> 64) as u64,
        129..=192 => {
            (num.low() >> (bin_length - 64)) as u64 | ((*num.high() as u64) << (192 - bin_length))
        }
        193.. => ((num.high() << (256 - bin_length)) >> 64) as u64,
    }
}

#[inline]
fn is_bin_palindrome(num: u256, bin_length: u32) -> bool {
    num.bits() == bin_length
        && top_64_bits(num, bin_length) == (*num.low() as u64).reverse_bits()
        && (bin_length <= 128
            || top_64_bits(num, bin_length - 64) == ((*num.low() >> 64) as u64).reverse_bits())
}

#[inline]
fn stabilized_bits(new_min_dec: u256, new_max_dec: u256, bin_length: u32) -> i32 {
    (bin_length as i32) - ((new_min_dec ^ new_max_dec).bits() as i32)
}

#[inline]
fn forward_key(num: u256, bin_length: u32, precision: u32) -> u64 {
    // depth < 64
    let top = *(num >> (bin_length - precision)).low() as u64;
    let required_low = top.reverse_bits() >> (64 - precision);
    let num_low = *num.low() as u64 & mask_lo(precision);
    required_low.wrapping_sub(num_low) & mask_lo(precision)
}

/// Probe key of a backward value: its low 64 bits reversed (the probe axis is the
/// reversed-key order, so completions sort by how they mirror the forward side).
#[inline]
fn reversed_backwards_key(c: u256) -> u64 {
    (*c.low() as u64).reverse_bits()
}

#[inline]
fn exploded_slot(key: u64, depth: u32, explosion_factor: usize) -> usize {
    // We shift by depth to remove the top bits which are shared for all values
    // in the bucket.
    ((key as usize) << depth)
        .carrying_mul(explosion_factor, 0)
        .1
}

struct Bucket {
    /// Actual numbers in the bucket, stretched across an oversized vector, such that every
    /// number is at or after its exploded slot.
    perimeter: Vec<u256>,

    /// Bitmap of keys
    bitmap: Bitmap,

    /// Factor used for
    explosion_factor: usize,
}

impl Bucket {
    fn new() -> Self {
        Self {
            perimeter: vec![],
            bitmap: Bitmap::empty(),
            explosion_factor: 0,
        }
    }

    fn rebuild(
        &mut self,
        digit_cache: &[[u256; 10]],
        forward_depth: u32,
        depth: u32,
        bucket_key: u64,
    ) -> u64 {
        let count = 5u64.pow(depth - forward_depth) as usize;
        self.explosion_factor = count + count / 4;
        self.perimeter.clear();
        // 64 elements slack
        self.perimeter.resize(self.explosion_factor + 64, u256::MAX);
        let perimeter_slice = self.perimeter.as_mut_slice();
        let atomic_perimeter_slice = unsafe {
            std::slice::from_raw_parts(
                perimeter_slice.as_ptr() as *const AtomicU64,
                self.perimeter.len() * 4,
            )
        };
        (0..count)
            .into_par_iter()
            // Create the number
            .map(|idx| {
                let mut idx = idx as u64;
                let mut partial = u256::ZERO;
                for i in forward_depth..depth {
                    let t = idx % 5;
                    idx /= 5;
                    let p_i = (*(partial >> i).low() as u64) & 1;
                    let target_i = (bucket_key >> i) & 1;
                    let parity = p_i ^ target_i;
                    let g = (2 * t + parity) as usize;
                    partial += digit_cache[i as usize][g];
                }
                partial
            })
            // Atomically put it into the first free spot after its exploded key
            .for_each(|number| {
                let key = reversed_backwards_key(number);
                let mut index = exploded_slot(key, depth, self.explosion_factor) * 4;
                let limbs = [
                    *number.low() as u64,
                    (*number.low() >> 64) as u64,
                    *number.high() as u64,
                    (*number.high() >> 64) as u64,
                ];
                assert_ne!(limbs[0], u64::MAX);
                while atomic_perimeter_slice[index]
                    .compare_exchange(u64::MAX, limbs[0], Ordering::Acquire, Ordering::Relaxed)
                    .is_err()
                {
                    index += 4;
                }
                for i in 1..=3 {
                    assert_eq!(
                        atomic_perimeter_slice[index + i].swap(limbs[i], Ordering::Relaxed),
                        u64::MAX
                    );
                }
            });

        // Split into slices that can be operated on in parallel
        let slices = {
            let block_count = rayon::current_num_threads() * 4;
            let mut block_indices = (0..block_count)
                .map(|i| self.perimeter.len() * i / block_count)
                .collect::<Vec<_>>();
            for start in block_indices.iter_mut().skip(1) {
                while *start > 0 && self.perimeter[*start - 1] != u256::MAX {
                    *start -= 1;
                }
            }
            block_indices.dedup();
            block_indices.push(self.perimeter.len());
            let mut perimeter_slice = self.perimeter.as_mut_slice();
            let mut slices = vec![];
            for i in 0..block_indices.len() - 1 {
                let start = block_indices[i];
                let end = block_indices[i + 1];
                let (block_slice, rest) = perimeter_slice.split_at_mut(end - start);
                slices.push(block_slice);
                perimeter_slice = rest;
            }

            slices
        };

        // Sort the entire thing, taking into account that we only need to sort stretches of
        // elements with no empty slots.
        slices.into_par_iter().for_each(|slice| {
            let mut start = 0;
            'outer: while start < slice.len() {
                while *slice[start].low() as u64 == u64::MAX {
                    start += 1;
                    if start >= slice.len() {
                        break 'outer;
                    }
                }
                let mut end = start + 1;
                while end < slice.len() && *slice[end].low() as u64 != u64::MAX {
                    end += 1;
                }
                if end > start + 1 {
                    slice[start..end].sort_unstable_by_key(|&c| reversed_backwards_key(c));
                }
                start = end + 1;
            }
        });

        // Recreate the bitmap.
        self.bitmap.rebuild(
            count,
            self.perimeter
                .iter()
                .filter(|&c| *c.low() as u64 != u64::MAX)
                .map(|&c| reversed_backwards_key(c)),
            depth,
        );
        count as u64
    }
}

/// Exact bitmap summary near the bucket's information density. A pair whose stabilized
/// key prefix is absent cannot have any candidates. Shorter prefixes query aligned ranges
/// in the bitmap; longer prefixes query the deepest stored ancestor. Both can admit false
/// positives but never false negatives. (An unreversed multi-level variant — one bitmap
/// per depth, single-bit queries, no reversal on the reject path — was tried 2026-07-17:
/// semantics-identical, but probe was a wash (the reversal was latency-hidden) and the
/// build's bit-set order turned random, costing ~15% of back. Reverted.)
struct Bitmap {
    d: u32,
    min_extra: u32,
    max_extra: u32,
    bits: Vec<u64>,
}

impl Bitmap {
    const EXTRA_LEVELS: u32 = 3;

    fn empty() -> Self {
        Self {
            d: 0,
            min_extra: 0,
            max_extra: 0,
            bits: vec![],
        }
    }

    /// Recompute in place, reusing the bitmap allocation across chunks.
    fn rebuild(&mut self, n: usize, keys: impl Iterator<Item = u64>, d: u32) {
        let floor_log2 = usize::BITS - 1 - n.leading_zeros();
        let max_extra = 64 - d;
        let min_extra = floor_log2.min(max_extra);
        let max_extra = (min_extra + Self::EXTRA_LEVELS).min(max_extra);
        self.bits.clear();
        self.bits.resize((1usize << max_extra).div_ceil(64), 0);
        for key in keys {
            let variable = key << d;
            let prefix = (variable >> (64 - max_extra)) as usize;
            self.bits[prefix >> 6] |= 1u64 << (prefix & 63);
        }
        self.d = d;
        self.min_extra = min_extra;
        self.max_extra = max_extra;
    }

    #[inline]
    fn may_contain(&self, sbits: u32, lo: u64) -> bool {
        let extra = sbits - self.d;
        if extra < self.min_extra {
            return true;
        }
        let query_extra = extra.min(self.max_extra);
        let prefix = ((lo << self.d) >> (64 - query_extra)) as usize;
        let shift = self.max_extra - query_extra;
        let start = prefix << shift;
        let width = 1usize << shift;
        let mask = mask_lo(width as u32) << (start & 63);
        self.bits[start >> 6] & mask != 0
    }
}

struct ForwardFront {
    /// Vectors of nodes with enough bits to determine a bucket according to their depths.
    arrived_by_depth: Vec<Vec<u256>>,
    /// Nodes which didn't stabilise enough bits before hitting the bucket.
    leftover: Vec<(u256, u256)>,
}

/// Expand nodes forward until enough bits stabilized to determine a bucket, up to a depth of
/// `forward_depth`.
fn forward_arrived(dec_length: u32, bin_length: u32, forward_depth: u32) -> ForwardFront {
    let depth = dec_length.div_ceil(2);
    let digit_cache = get_digit_cache(dec_length);
    let max_dec_cache = get_max_cache(dec_length, 10);
    let max_bin_cache = get_max_cache(bin_length, 2);
    let mut arrived_by_depth: Vec<Vec<u256>> = vec![vec![]; forward_depth as usize + 1];
    let mut frontier = vec![(u256::ZERO, u256::ZERO)];
    for level in 0..forward_depth {
        let max_bin_add = max_bin_cache[level as usize];
        let max_dec_add = max_dec_cache[level as usize];
        let mut next = vec![];
        let mut arrived_here: Vec<(u256, u256)> = vec![];
        for &(current_num, bin_num) in &frontier {
            let start = (level == 0) as usize;
            let step = if level == 0 { 2 } else { 1 };
            for digit in (start..=9).step_by(step) {
                let new_num = current_num + digit_cache[level as usize][digit];
                let new_bin_num = bin_num + (((new_num >> level) & 1) << (bin_length - level - 1));
                let new_max_dec = new_num + max_dec_add;
                if new_bin_num + max_bin_add < new_num || new_max_dec < new_bin_num {
                    continue;
                }

                let stabilized = stabilized_bits(new_num, new_max_dec, bin_length);
                if stabilized >= depth as i32 {
                    arrived_here.push((new_num, new_bin_num));
                } else {
                    next.push((new_num, new_bin_num));
                }
            }
        }

        let middle_size = 10u128.pow(forward_depth - level - 1);
        let count = arrived_here.len() as u128;
        // We don't want to create more middle nodes than the amount we save by not
        // expanding the current nodes further.
        if count + middle_size > 5 * count {
            next.extend(arrived_here);
        } else {
            arrived_by_depth[(level + 1) as usize]
                .extend(arrived_here.into_iter().map(|(num, _)| num));
        }
        frontier = next;
    }
    ForwardFront {
        arrived_by_depth,
        leftover: frontier,
    }
}

struct Middle {
    // key -> first index of value in `values` with this key
    indices: Vec<u32>,
    values: Vec<u256>,
}

impl Middle {
    fn new() -> Self {
        Middle {
            indices: vec![],
            values: vec![],
        }
    }

    fn rebuild(
        &mut self,
        digit_cache: &[[u256; 10]],
        arrived_depth: u32,
        forward_depth: u32,
        depth: u32,
    ) {
        let digits = (forward_depth - arrived_depth) as usize;
        let mid_count = 10usize.pow(digits as u32);
        self.values.clear();
        self.values.reserve_exact(mid_count);
        (0..mid_count).into_iter().for_each(|mut idx| {
            let mut val = u256::ZERO;
            for j in 0..digits {
                let dig = idx % 10;
                idx /= 10;
                val += digit_cache[arrived_depth as usize + j][dig];
            }
            self.values.push(val);
        });

        let key_mask = mask_lo(depth);
        let key_count = 1usize << (depth - arrived_depth);
        self.indices.clear();
        self.indices.resize(key_count + 1, 0);
        self.values
            .sort_unstable_by_key(|mid| (*mid.low() as u64 & key_mask) >> arrived_depth);
        for mid in &self.values {
            let key = (*mid.low() as u64 & key_mask) >> arrived_depth;
            self.indices[key as usize + 1] += 1;
        }
        for r in 0..key_count {
            self.indices[r + 1] += self.indices[r];
        }
    }

    fn get_for_key(&self, key: usize) -> &[u256] {
        &self.values[self.indices[key] as usize..self.indices[key + 1] as usize]
    }
}

struct DecContext {
    d: u32,
    f: u32,
    mask_d: u64,
    digit_cache: Vec<[u256; 10]>,
    max_dec_cache: Vec<u256>,
    middles: Vec<Middle>,
    bin_contexts: Vec<BinContext>,
}

impl DecContext {
    fn new(dec_length: u32, b: u32, bins: Vec<u32>) -> Self {
        let d = dec_length.div_ceil(2);
        let f = d - b;
        let digit_cache = get_digit_cache(dec_length);
        let max_dec_cache = get_max_cache(dec_length, 10);
        let mask_d = mask_lo(d);

        let mut k_used = vec![false; f as usize + 1];
        let mut bin_contexts = vec![];
        for &bin_length in &bins {
            let ForwardFront {
                arrived_by_depth,
                leftover,
            } = forward_arrived(dec_length, bin_length, f);
            for k in 1..=f as usize {
                if arrived_by_depth[k].is_empty() {
                    continue;
                }
                k_used[k] = true;
            }

            let bin_context = BinContext {
                bin_length,
                arrived_by_depth,
                leftover,
            };
            bin_contexts.push(bin_context);
        }
        let mut middles: Vec<Middle> = (0..=f as usize).map(|_| Middle::new()).collect();
        for k in 1..=f as usize {
            if k_used[k] {
                middles[k].rebuild(&digit_cache, k as u32, f, d);
            }
        }

        Self {
            d,
            f,
            mask_d,
            digit_cache,
            max_dec_cache,
            middles,
            bin_contexts,
        }
    }

    fn expand(self) -> Self {
        let digit_cache = &self.digit_cache;
        let mut new_bin_contexts = vec![];
        let next_f = self.f + 1;
        for bin_context in self.bin_contexts {
            let bin_length = bin_context.bin_length;
            let max_bin_add = get_max_cache(bin_length, 2)[self.f as usize];
            let max_dec_add = self.max_dec_cache[self.f as usize];
            // Expand one level with the range prune, splitting off arrivals at depth k (the
            // same split criterion as forward_arrived). The frontier here is tiny — a few
            // thousand never-arrived nodes at most — so plain sequential loops.
            let mut arrived: Vec<u256> = vec![];
            let mut leftover: Vec<(u256, u256)> = vec![];
            for &(current_num, bin_num) in &bin_context.leftover {
                for next_digit_add in digit_cache[self.f as usize] {
                    let new_num = current_num + next_digit_add;
                    let new_bin_num =
                        bin_num + (((new_num >> self.f) & 1) << (bin_length - self.f - 1));
                    let new_max_dec = new_num + max_dec_add;
                    if new_bin_num + max_bin_add < new_num || new_max_dec < new_bin_num {
                        continue;
                    }
                    let stabilized = bin_length as i32 - (new_num ^ new_max_dec).bits() as i32;
                    if stabilized >= self.d as i32 {
                        arrived.push(new_num);
                    } else {
                        leftover.push((new_num, new_bin_num));
                    }
                }
            }
            let mut arrived_by_depth = vec![vec![]; next_f as usize];
            arrived_by_depth.push(arrived);
            let new_bin_context = BinContext {
                bin_length,
                arrived_by_depth,
                leftover,
            };
            new_bin_contexts.push(new_bin_context);
        }
        let mut middles: Vec<Middle> = (0..=next_f).map(|_| Middle::new()).collect();
        middles[next_f as usize].rebuild(&self.digit_cache, next_f, next_f, self.d);
        DecContext {
            f: next_f,
            middles,
            bin_contexts: new_bin_contexts,
            ..self
        }
    }
}

struct BinContext {
    bin_length: u32,
    arrived_by_depth: Vec<Vec<u256>>,
    leftover: Vec<(u256, u256)>,
}

#[inline]
fn scan_perimeter(
    scan_start: u64,
    scan_end: u64,
    fwd: u256,
    perimeter: &[u256],
    bin_length: u32,
    d: u32,
    explosion_factor: usize,
    found: &mut Vec<u256>,
    cand: &mut u64,
) {
    let mut q = exploded_slot(scan_start, d, explosion_factor);
    while q < perimeter.len() {
        let back = perimeter[q];
        if *back.low() as u64 == u64::MAX {
            q += 1;
            continue;
        }
        let back_key = reversed_backwards_key(back);
        if back_key < scan_start {
            q += 1;
            continue;
        }
        if back_key > scan_end {
            break;
        }
        *cand += 1;
        let n = fwd + back;
        q += 1;
        if is_bin_palindrome(n, bin_length) {
            found.push(n);
        }
    }
}

/// Per-node probe output: (found, pairs, cand, skipped).
type ProbeAcc = (Vec<u256>, u64, u64, u64);

fn merge_acc(mut a: ProbeAcc, mut b: ProbeAcc) -> ProbeAcc {
    a.0.append(&mut b.0);
    a.1 += b.1;
    a.2 += b.2;
    a.3 += b.3;
    a
}

struct JoinArgs<'p> {
    bucket_key: u64,
    mask_d: u64,
    d: u32,
    bin_length: u32,
    max_dec_add: u256,
    middles: &'p [Middle],
    bucket: &'p Bucket,
}

impl JoinArgs<'_> {
    /// Tries to join `arrived` with the bucket by generating all sums of `arrived` with the
    /// different middle nodes that would fall in the bucket, and then for each one calculating
    /// the expected key in the bucket, first verifying against the bitmap and finally scanning
    /// the relevant slice of the bucket.
    #[inline(always)]
    fn join(&self, arrived_depth: usize, arrived_key: u64, arrived: u256) -> ProbeAcc {
        let required_key = arrived_key.wrapping_sub(self.bucket_key) & self.mask_d;
        let mut local_found: Vec<u256> = Vec::new();
        let (mut local_pairs, mut local_cand, mut local_skipped) = (0u64, 0u64, 0u64);
        let middle = &self.middles[arrived_depth];
        let middle_key = (required_key >> arrived_depth) as usize;
        let mids = middle.get_for_key(middle_key);
        local_pairs += mids.len() as u64;
        for &mid in mids {
            let fwd = arrived + mid;
            let new_max_dec = fwd + self.max_dec_add;
            let stabilized = stabilized_bits(fwd, new_max_dec, self.bin_length).min(64) as u32;
            debug_assert!(
                stabilized < self.d,
                "too few stabilized bits: {stabilized} < {}",
                self.d
            );
            let key_s = forward_key(fwd, self.bin_length, stabilized);
            let r = key_s.reverse_bits() >> (64 - stabilized);
            let scan_start = r << (64 - stabilized); // < 2^64

            if !self.bucket.bitmap.may_contain(stabilized, scan_start) {
                local_skipped += 1;
                continue;
            }
            let scan_end = ((r + 1) << (64 - stabilized)) - 1;
            scan_perimeter(
                scan_start,
                scan_end,
                fwd,
                &self.bucket.perimeter,
                self.bin_length,
                self.d,
                self.bucket.explosion_factor,
                &mut local_found,
                &mut local_cand,
            );
        }
        (local_found, local_pairs, local_cand, local_skipped)
    }
}

fn join_bucket(
    ctx: &DecContext,
    bucket_key: u64,
    found: &mut Vec<u256>,
    bucket: &mut Bucket,
) -> (u64, f64, u64, u64, u64) {
    let (d, f, mask_d) = (ctx.d, ctx.f, ctx.mask_d);
    let max_dec_cache = &ctx.max_dec_cache;
    let middles = &ctx.middles;

    let t_b = Instant::now();
    let back_gen = bucket.rebuild(&ctx.digit_cache, f, d, bucket_key);
    let t_back = t_b.elapsed().as_secs_f64();
    let (mut pairs_t, mut cand_t, mut skipped_t) = (0u64, 0u64, 0u64);
    let max_dec_add = max_dec_cache[f as usize - 1];
    for bin_context in &ctx.bin_contexts {
        let args = JoinArgs {
            bucket_key,
            mask_d,
            d,
            bin_length: bin_context.bin_length,
            max_dec_add,
            middles,
            bucket,
        };
        let args = &args;
        for (arrived_depth, arrived) in bin_context.arrived_by_depth.iter().enumerate() {
            let (found_chunk, pairs, cand, skipped) = arrived
                .par_iter()
                .map(|&arrived| {
                    args.join(
                        arrived_depth,
                        forward_key(arrived, bin_context.bin_length, d),
                        arrived,
                    )
                })
                .reduce(|| (Vec::new(), 0u64, 0u64, 0u64), merge_acc);
            found.extend(found_chunk);

            pairs_t += pairs;
            cand_t += cand;
            skipped_t += skipped;
        }
    }
    (back_gen, t_back, pairs_t, cand_t, skipped_t)
}

fn deeper_passes(ctx0: &DecContext, found: &mut Vec<u256>) {
    let (d, mask_d) = (ctx0.d, ctx0.mask_d);
    let bin_contexts = ctx0
        .bin_contexts
        .iter()
        .map(|bin_context| BinContext {
            bin_length: bin_context.bin_length,
            arrived_by_depth: vec![],
            leftover: bin_context.leftover.clone(),
        })
        .collect();
    let mut ctx = DecContext {
        d,
        f: ctx0.f,
        mask_d,
        digit_cache: ctx0.digit_cache.clone(),
        max_dec_cache: ctx0.max_dec_cache.clone(),
        middles: vec![],
        bin_contexts,
    };
    let mut bucket = Bucket::new();
    loop {
        if ctx
            .bin_contexts
            .iter()
            .all(|bin_context| bin_context.leftover.is_empty())
        {
            return;
        }
        if ctx.f == d - 1 {
            // Last level: one digit remains — verify leftover nodes and spilled pairs
            // directly.
            let digit_cache = &ctx.digit_cache;
            for bin_context in &ctx.bin_contexts {
                let bin_length = bin_context.bin_length;
                let level_found: Vec<u256> = bin_context
                    .leftover
                    .par_iter()
                    .map(|&(num, _)| num)
                    .flat_map_iter(|num| {
                        (0..=9usize).filter_map(move |digit| {
                            let n = num + digit_cache[(d - 1) as usize][digit];
                            is_bin_palindrome(n, bin_length).then_some(n)
                        })
                    })
                    .collect();
                found.extend(level_found);
            }
            return;
        }
        ctx = ctx.expand();
        let mut keys: Vec<u64> = Vec::new();
        for bin_context in &ctx.bin_contexts {
            keys.extend(
                bin_context
                    .arrived_by_depth
                    .last()
                    .unwrap()
                    .iter()
                    .map(|&a| forward_key(a, bin_context.bin_length, d)),
            );
        }
        keys.sort_unstable();
        keys.dedup();
        for &bucket_key in &keys {
            join_bucket(&ctx, bucket_key, found, &mut bucket);
        }
    }
}

/// Try one bucket for each candidate b, and pick the faster one.
fn pick_backward_depth(dec_length: u32, bins: Vec<u32>, verbose: bool) -> (u32, DecContext) {
    let d = dec_length.div_ceil(2);
    let b0 = default_backward_depth(dec_length).max(1).min(d - 1);
    let mut cands: Vec<(u32, DecContext, f64)> = vec![];
    let mut bucket = Bucket::new();
    let bs = if b0 > 1 { vec![b0 - 1, b0] } else { vec![b0] };
    for cb in bs {
        let nchunks = 1u64 << cb;
        let t0 = Instant::now();
        let ctx = DecContext::new(dec_length, cb, bins.clone());
        let t_setup = t0.elapsed().as_secs_f64();
        let mut scratch: Vec<u256> = vec![];
        let t1 = Instant::now();
        join_bucket(&ctx, (nchunks / 2) << ctx.f, &mut scratch, &mut bucket);

        let est = t_setup + t1.elapsed().as_secs_f64() * nchunks as f64;
        if verbose {
            println!(
                "  pick_b dec={dec_length}: b={cb} gauge est={est:.3}s (setup {t_setup:.3} + 1 chunk x {nchunks})"
            );
        }
        cands.push((cb, ctx, est));
    }
    let (b, ctx, _) = cands
        .into_iter()
        .min_by(|x, y| x.2.total_cmp(&y.2))
        .unwrap();
    (b, ctx)
}

fn find_brute_force(dec_length: u32) -> Vec<u256> {
    let mut result = vec![];
    for i in 10u32.pow(dec_length - 1)..10u32.pow(dec_length) {
        let dec_str = i.to_string();
        let num = u256::from(i);
        if dec_str.chars().eq(dec_str.chars().rev()) && is_bin_palindrome(num, num.bits()) {
            result.push(u256::from(i));
        }
    }
    result
}

fn find_dec_length(dec_length: u32) -> Vec<u256> {
    let d = dec_length.div_ceil(2);
    if d < 2 {
        return find_brute_force(dec_length);
    }
    let max_bin = (u256::from(10u32).pow(dec_length) - 1).bits();
    let min_bin = (u256::from(10u32).pow(dec_length - 1) + 1).bits();
    let bins: Vec<u32> = (min_bin..=max_bin).collect();
    let (b, ctx) = pick_backward_depth(dec_length, bins, false);

    let mut found: Vec<u256> = vec![];

    {
        let bar = ProgressBar::new(1u64 << b).with_style(
            ProgressStyle::with_template("{wide_bar} {pos}/{len} (ETA: {eta_precise})").unwrap(),
        );

        let mut bucket = Bucket::new();
        for w in 0..(1u64 << b) {
            join_bucket(&ctx, w << ctx.f, &mut found, &mut bucket);
            bar.inc(1);
        }
        bar.finish();
    }

    deeper_passes(&ctx, &mut found);

    found.sort_unstable();
    found.dedup();
    found
}

fn main() {
    let start_length = if std::env::args().len() > 1 {
        std::env::args().nth(1).unwrap().parse().unwrap()
    } else {
        1
    };

    let start_time = Instant::now();
    let mut ordinal = 1u64;
    for dec_length in start_length..=u32::MAX {
        let found = find_dec_length(dec_length);
        for n in &found {
            ordinal += 1;
            println!("{ordinal} {n}");
        }
        println!(
            "{:.4}: finished decimal length {dec_length} ({} found)",
            start_time.elapsed().as_secs_f32(),
            found.len()
        );
    }
}
