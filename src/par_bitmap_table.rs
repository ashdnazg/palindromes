use std::sync::atomic::{AtomicU64, Ordering};

use ethnum::u256;
use rayon::{
    iter::plumbing::{bridge_unindexed, UnindexedConsumer, UnindexedProducer},
    prelude::*,
};

use crate::Bits as _;

struct DigitRange<'a> {
    start: u64,
    end: u64,
    digit_cache_64: &'a [[u64; 10]],
}

impl<'a> DigitRange<'a> {
    fn new(digit_cache_64: &'a [[u64; 10]]) -> Self {
        Self {
            digit_cache_64,
            start: 0,
            end: 10u64.pow(digit_cache_64.len() as u32) - 1,
        }
    }
}

impl UnindexedProducer for DigitRange<'_> {
    type Item = u64;

    fn split(mut self) -> (Self, Option<Self>) {
        let split_point = self.start + (self.end - self.start) / 2;
        if split_point == self.start {
            return (self, None);
        }

        let other = Self {
            digit_cache_64: self.digit_cache_64,
            start: split_point + 1,
            end: self.end,
        };

        self.end = split_point;

        (self, Some(other))
    }

    fn fold_with<F>(mut self, folder: F) -> F
    where
        F: rayon::iter::plumbing::Folder<Self::Item>,
    {
        let mut state = vec![(0usize, 0u64); self.digit_cache_64.len() + 1];
        let mut end = vec![0; self.digit_cache_64.len()];
        for i in 0..end.len() {
            let start_digit = self.start % 10;
            let end_digit = self.end % 10;
            self.start /= 10;
            self.end /= 10;
            state[i].0 = start_digit as usize;
            end[i] = end_digit as usize;
        }
        for i in (1..state.len()).rev().skip(1) {
            state[i].1 = state[i + 1]
                .1
                .wrapping_add(self.digit_cache_64[i][state[i].0]);
        }

        let iter = DigitIterator {
            digit_cache_64: self.digit_cache_64,
            state: Some(state),
            end,
        };
        folder.consume_iter(iter)
    }
}

struct DigitIterator<'a> {
    digit_cache_64: &'a [[u64; 10]],
    state: Option<Vec<(usize, u64)>>,
    end: Vec<usize>,
}

impl std::iter::Iterator for DigitIterator<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        let stack = self.state.as_mut()?;

        let sum = stack[1].1.wrapping_add(self.digit_cache_64[0][stack[0].0]);
        let ret = sum.reverse_bits();

        let mut plus_idx = 0;
        loop {
            if stack.iter().zip(self.end.iter()).all(|((i, _), j)| i == j) {
                self.state = None;
                return Some(ret);
            }

            if stack[plus_idx].0 < 9 {
                stack[plus_idx].0 += 1;
                break;
            }

            stack[plus_idx].0 = 0;
            plus_idx += 1;
        }

        while plus_idx > 0 {
            stack[plus_idx].1 = stack[plus_idx + 1]
                .1
                .wrapping_add(self.digit_cache_64[plus_idx][stack[plus_idx].0]);
            plus_idx -= 1;
        }

        Some(ret)
    }
}

impl ParallelIterator for DigitRange<'_> {
    type Item = u64;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge_unindexed(self, consumer)
    }
}

#[inline(never)]
pub fn get_digit_cache_64(digit_cache: &[[u256; 10]], level: usize) -> Vec<[u64; 10]> {
    digit_cache
        .iter()
        .skip(level)
        .map(|c| c.map(|n| *(n >> level).low() as u64))
        .collect()
}

#[derive(Clone, Debug)]
pub struct LevelTable {
    bitmap: Vec<u64>,
    min_lookup_bits: u32,
    max_lookup_bits: u32,
}

impl LevelTable {
    fn insert(&self, num: u64) {
        let entry = num.wrapping_shr(u64::BITS - self.max_lookup_bits);
        let atomic_ref =
            unsafe { &*(&self.bitmap[entry as usize / 64] as *const u64 as *const AtomicU64) };
        atomic_ref.fetch_or(1u64.wrapping_shl(entry as u32 % 64), Ordering::Relaxed);
    }

    fn contains(&self, num: u64, known_bits: u32) -> bool {
        let known_bits = known_bits.min(self.max_lookup_bits);
        let entry = num
            .wrapping_shr(u64::BITS - known_bits)
            .wrapping_shl(self.max_lookup_bits - known_bits);
        let mask_bits = 1u32.wrapping_shl(self.max_lookup_bits - known_bits);
        let mask = (1u128.wrapping_shl(mask_bits) as u64).wrapping_sub(1);
        let shifted_mask = mask.wrapping_shl(entry as u32 % 64);
        self.bitmap[entry as usize / 64] & shifted_mask != 0
    }

    fn populate(&mut self, level: usize, digit_cache: &[[u256; 10]]) {
        let digit_cache_64 = get_digit_cache_64(digit_cache, level);
        DigitRange::new(&digit_cache_64).for_each(|n| {
            self.insert(n);
        });
    }

    pub fn calculate_memory_requirements(
        num_digits: u32,
        available_memory: u64,
    ) -> Option<(u32, u64)> {
        let sub_cache_size = 10u64.pow(num_digits);
        for downscale_factor in 0..=6 {
            let length =
                sub_cache_size.next_power_of_two() as usize / 1usize.wrapping_shl(downscale_factor);
            let size = (length * size_of::<u64>()) as u64;
            if size as u64 <= available_memory {
                return Some((downscale_factor, size));
            }
        }

        None
    }

    pub fn size(&self) -> usize {
        self.bitmap.len() * size_of::<u64>()
    }

    #[allow(dead_code)]
    pub fn saturation(&self) -> f64 {
        let mut count = 0u64;
        for &word in &self.bitmap {
            count += word.count_ones() as u64;
        }

        count as f64 / (self.bitmap.len() * u64::BITS as usize) as f64
    }

    fn new(num_digits: u32, downscale_factor: u32, digit_cache: &[[u256; 10]]) -> Option<Self> {
        let sub_cache_size = 10u64.pow(num_digits);
        if sub_cache_size < 64 {
            return None;
        }

        let length =
            sub_cache_size.next_power_of_two() as usize / 1usize.wrapping_shl(downscale_factor);
        assert_eq!(
            length.ilog2() + 6,
            sub_cache_size.bits() + (6 - downscale_factor)
        );

        let mut instance = Self {
            bitmap: vec![0; length],
            min_lookup_bits: length.ilog2(),
            max_lookup_bits: length.ilog2() + 6,
        };

        let level = digit_cache.len() as u32 - num_digits;

        instance.populate(level as usize, digit_cache);

        Some(instance)
    }

    fn lookup(&self, current_num: u256, level: u32, known_bits: u32, bin_length: u32) -> bool {
        if known_bits < self.min_lookup_bits {
            return true;
        }

        let current_bits = *(current_num >> level).low() as u64;
        let shift = bin_length as i32 - level as i32 - 64;
        let final_bits = (if shift > 0 {
            *(current_num >> shift).low() as u64
        } else {
            (*current_num.low() as u64) << -shift
        })
        .reverse_bits();

        let lookup_bits = final_bits.wrapping_sub(current_bits).reverse_bits();
        self.contains(lookup_bits, known_bits)
    }
}

#[derive(Clone, Debug)]
pub struct LookupTable {
    // index is the recursion level.
    pub sub_caches: Vec<Option<LevelTable>>,
}

impl LookupTable {
    pub fn new(digit_cache: &[[u256; 10]]) -> Self {
        Self {
            sub_caches: vec![None; digit_cache.len()],
        }
    }

    pub fn generate(
        &mut self,
        num_digits: u32,
        downscale_factor: u32,
        digit_cache: &[[u256; 10]],
    ) -> bool {
        if num_digits as usize > digit_cache.len() {
            return false;
        }

        let level = digit_cache.len() - num_digits as usize;
        if let Some(level_table) = LevelTable::new(num_digits, downscale_factor, digit_cache) {
            self.sub_caches[level] = Some(level_table);
        }

        true
    }

    pub fn lookup(
        &self,
        current_num: u256,
        msb_set_bits: i32,
        level: u32,
        bin_length: u32,
    ) -> bool {
        if (level as i32) > msb_set_bits {
            return true;
        }

        self.sub_caches[level as usize]
            .as_ref()
            .map_or(true, |level_table| {
                level_table.lookup(
                    current_num,
                    level,
                    (msb_set_bits as u32) - level,
                    bin_length,
                )
            })
    }
}
