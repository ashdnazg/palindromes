use ethnum::u256;
use hashbrown::HashSet;
use once_cell::sync::OnceCell;
use std::{
    ops::Range,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
    time::Instant,
};

trait Bits {
    fn bits(&self) -> u32;
}

impl Bits for u256 {
    fn bits(&self) -> u32 {
        256 - self.leading_zeros()
    }
}

fn encode_ten_thousands(hi: u64, lo: u64) -> u64 {
    let merged: u64 = hi | (lo << 32);
    let top = ((merged * 10486u64) >> 20) & ((0x7Fu64 << 32) | 0x7Fu64);
    let bot = merged - 100u64 * top;
    let mut tens;
    let hundreds = (bot << 16) + top;
    tens = (hundreds * 103u64) >> 10;
    tens &= (0xFu64 << 48) | (0xFu64 << 32) | (0xFu64 << 16) | 0xFu64;
    tens += (hundreds - 10u64 * tens) << 8;

    tens
}

fn to_digits(x: u64) -> u128 {
    let top = x / 100000000;
    let bottom = x % 100000000;
    let first = encode_ten_thousands(top / 10000, top % 10000);
    let second = encode_ten_thousands(bottom / 10000, bottom % 10000);
    ((second as u128) << 64) | (first as u128)
}

#[derive(Clone, Debug)]
struct LevelTable {
    // index is number of known bits.
    sub_caches: Vec<OnceCell<HashSet<u256>>>,
}

impl LevelTable {
    fn new(num_digits: u32, digit_cache: &[u256]) -> Self {
        let level = digit_cache.len() as u32 - num_digits;
        let max_known_bits = digit_cache
            .iter()
            .skip(level as usize)
            .map(|n| n * 9)
            .sum::<u256>()
            .bits();

        Self {
            sub_caches: vec![OnceCell::new(); (max_known_bits + 1) as usize],
        }
    }

    fn generate(&self, num_digits: u32, digit_cache: &[u256], cancel_marker: &AtomicBool) {
        let level = digit_cache.len() as u32 - num_digits;
        let min_known_bits = u256::from(10u32).pow(num_digits).bits();
        let max_known_bits = digit_cache
            .iter()
            .skip(level as usize)
            .map(|n| n * 9)
            .sum::<u256>()
            .bits();
        let sub_cache_size = 10u64.pow(num_digits);

        if cancel_marker.load(Ordering::Acquire) {
            return;
        }

        let mut sub_caches = vec![HashSet::new(); (max_known_bits + 1) as usize]; //Vec::with_capacity((max_known_bits - min_known_bits + 1) as usize);

        for n in 0..sub_cache_size {
            if cancel_marker.load(Ordering::Acquire) {
                return;
            }
            let digits = to_digits(n);
            let mut sum = u256::ZERO;
            for i in 0..num_digits {
                let digit = (digits >> (120 - i * 8)) & 15;
                sum += (digit_cache[(level + i) as usize] * digit) >> level;
            }
            for known_bits in min_known_bits..(max_known_bits + 1) {
                sub_caches[known_bits as usize].insert(sum & ((1 << known_bits) - 1));
            }
        }

        for (i, sub_cache) in sub_caches.into_iter().enumerate() {
            if cancel_marker.load(Ordering::Acquire) {
                return;
            }
            if sub_cache.is_empty() {
                continue;
            }

            self.sub_caches[i].set(sub_cache).unwrap();
        }
    }

    fn lookup(&self, current_num: u256, level: u32, known_bits: u32, bin_length: u32) -> bool {
        if let Some(sub_cache) = self.sub_caches[known_bits as usize].get() {
            let mask = (u256::ONE << known_bits) - 1;
            let current_bits = current_num >> level;
            let final_bits = current_num.reverse_bits() >> (256 + level - bin_length);
            let lookup_bits = final_bits.wrapping_sub(current_bits) & mask;

            let ret = sub_cache.contains(&lookup_bits);
            return ret;
        }
        true
    }
}

#[derive(Clone, Debug)]
struct LookupTable {
    // index is the recursion level.
    sub_caches: Vec<OnceCell<LevelTable>>,
}

impl LookupTable {
    fn new(digit_cache: &[u256]) -> Self {
        Self {
            sub_caches: vec![OnceCell::new(); digit_cache.len()],
        }
    }

    fn generate(&self, num_digits: u32, digit_cache: &[u256], cancel_marker: &AtomicBool) {
        if num_digits as usize > digit_cache.len() {
            return;
        }

        if cancel_marker.load(Ordering::Acquire) {
            return;
        }

        let level = digit_cache.len() - num_digits as usize;
        let level_table = LevelTable::new(num_digits, digit_cache);
        self.sub_caches[level]
            .try_insert(level_table)
            .unwrap()
            .generate(num_digits, digit_cache, cancel_marker);
    }

    fn lookup(&self, current_num: u256, max_dec: u256, level: u32, bin_length: u32) -> bool {
        let msb_set_bits = bin_length - (max_dec ^ current_num).bits();
        if level > msb_set_bits {
            return true;
        }

        self.sub_caches[level as usize]
            .get()
            .map_or(true, |level_table| {
                let known_bits = u32::min(
                    msb_set_bits - level,
                    level_table.sub_caches.len() as u32 - 1,
                );
                level_table.lookup(current_num, level, known_bits, bin_length)
            })
    }
}

fn find_palindrome_recursive(
    current_num: u256,
    bin_num: u256,
    level: u32,
    digits: impl Iterator<Item = u32>,
    dec_length: u32,
    bin_length: u32,
    digit_cache: &[u256],
    max_dec_cache: &[u256],
    max_bin_cache: &[u256],
    lookup_table: &LookupTable,
    start_time: Instant,
) {
    if (level + 1) * 2 >= dec_length {
        for digit in digits {
            let new_num = current_num + digit_cache[level as usize] * digit as u128;
            let leading_zeros = new_num.leading_zeros();
            if leading_zeros + bin_length != u256::BITS {
                continue;
            }

            let reversed = new_num.reverse_bits() >> leading_zeros;
            if reversed == new_num {
                println!("{:.2}: {}", start_time.elapsed().as_secs_f32(), new_num);
            }
        }

        return;
    }

    let max_bin_add = max_bin_cache[level as usize];
    let max_dec_add = max_dec_cache[level as usize];

    for digit in digits {
        let new_num = current_num + digit_cache[level as usize] * digit as u128;
        let new_bin_num = bin_num + (((new_num >> level) & 1) << (bin_length - level - 1));

        let new_max_dec = new_num + max_dec_add;

        if new_bin_num + max_bin_add < new_num || new_max_dec < new_bin_num {
            continue;
        }

        if !lookup_table.lookup(new_num, new_max_dec, level + 1, bin_length) {
            continue;
        }

        find_palindrome_recursive(
            new_num,
            new_bin_num,
            level + 1,
            0..10,
            dec_length,
            bin_length,
            digit_cache,
            max_dec_cache,
            max_bin_cache,
            lookup_table,
            start_time,
        );
    }
}

fn find_palindrome_internal(
    dec_length: u32,
    bin_length: u32,
    digit_cache: &[u256],
    max_dec_cache: &[u256],
    lookup_table: &LookupTable,
    start_time: Instant,
) {
    let max_bin_cache = get_max_cache(bin_length, 2);
    find_palindrome_recursive(
        u256::ZERO,
        u256::ZERO,
        0,
        (1..10).step_by(2),
        dec_length,
        bin_length,
        digit_cache,
        max_dec_cache,
        &max_bin_cache,
        lookup_table,
        start_time,
    );
}

fn find_palindrome(dec_length: u32, start_time: Instant) {
    let max_bin_length = (u256::from(10u32).pow(dec_length) - 1).bits();
    let min_bin_length = (u256::from(10u32).pow(dec_length - 1) + 1).bits();
    let digit_cache = get_digit_cache(dec_length);
    let max_dec_cache = get_max_cache(dec_length, 10);
    let lookup_table = LookupTable::new(&digit_cache);
    let cancel = AtomicBool::new(false);
    let finished_count = AtomicU32::new(0);

    rayon::scope_fifo(|scope| {
        for bin_length in min_bin_length..=max_bin_length {
            let digit_cache_ref = &digit_cache;
            let max_dec_cache_ref = &max_dec_cache;
            let lookup_table_ref = &lookup_table;
            let finished_count_ref = &finished_count;
            let cancel_ref = &cancel;
            scope.spawn_fifo(move |_| {
                find_palindrome_internal(
                    dec_length,
                    bin_length,
                    digit_cache_ref,
                    max_dec_cache_ref,
                    lookup_table_ref,
                    start_time,
                );
                if finished_count_ref.fetch_add(1, Ordering::Release)
                    == (max_bin_length - min_bin_length)
                {
                    cancel_ref.store(true, Ordering::Relaxed);
                }
            });
        }

        for num_digits in 2..8 {
            let digit_cache_ref = &digit_cache;
            let cancel_ref = &cancel;
            let lookup_table_ref = &lookup_table;
            scope.spawn_fifo(move |_| {
                lookup_table_ref.generate(num_digits, digit_cache_ref, cancel_ref);
            })
        }
    })
}

fn get_max_cache(length: u32, base: u32) -> Vec<u256> {
    let cache_length = (length + 1) / 2;
    (1..cache_length)
        .map(|i| u256::from(base).pow(length - i) - u256::from(base).pow(i))
        .collect()
}

fn get_digit_cache(dec_length: u32) -> Vec<u256> {
    let cache_length = (dec_length + 1) / 2;
    (0..cache_length)
        .map(|i| {
            let j = dec_length - i - 1;
            let mut entry = u256::from(10u32).pow(i);
            if i != j {
                entry += u256::from(10u32).pow(j);
            }

            entry
        })
        .collect()
}

fn main() {
    let start_time = Instant::now();
    let mut dec_length = 2;
    loop {
        find_palindrome(dec_length, start_time);
        dec_length += 1;
    }

    // let digit_cache = get_digit_cache(10);

    // let level_table = LevelTable::new(3, &digit_cache);

    // println!("{:?}", level_table.sub_caches);
}
