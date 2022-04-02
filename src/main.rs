use ethnum::u256;
use std::time::Instant;

trait Bits {
    fn bits(&self) -> u32;
}

impl Bits for u256 {
    fn bits(&self) -> u32 {
        256 - self.leading_zeros()
    }
}

fn find_palindrome_recursive<'a, 'b>(
    scope: &'a rayon::Scope<'b>,
    current_num: u256,
    bin_num: u256,
    level: u32,
    digits: impl Iterator<Item = u32>,
    dec_length: u32,
    bin_length: u32,
    digit_cache: &'b Vec<u256>,
    max_dec_cache: &'b Vec<u256>,
    max_bin_cache: &'b Vec<u256>,
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

        if new_bin_num + max_bin_add < new_num || new_num + max_dec_add < new_bin_num {
            continue;
        }
        scope.spawn(move |scope| {
            find_palindrome_recursive(
                scope,
                new_num,
                new_bin_num,
                level + 1,
                0..10,
                dec_length,
                bin_length,
                digit_cache,
                max_dec_cache,
                max_bin_cache,
                start_time,
            );
        });
    }
}

fn find_palindrome_internal<'a, 'b>(
    dec_length: u32,
    bin_length: u32,
    digit_cache: &'b Vec<u256>,
    max_dec_cache: &'b Vec<u256>,
    start_time: Instant,
) {
    let max_bin_cache = get_max_cache(bin_length, 2);
    rayon::scope(|scope| {
        find_palindrome_recursive(
            scope,
            u256::ZERO,
            u256::ZERO,
            0,
            (1..10).step_by(2),
            dec_length,
            bin_length,
            digit_cache,
            max_dec_cache,
            &max_bin_cache,
            start_time,
        );
    })
}

fn find_palindrome(dec_length: u32, start_time: Instant) {
    let max_bin_length = (u256::from(10u32).pow(dec_length) - 1).bits();
    let min_bin_length = (u256::from(10u32).pow(dec_length - 1) + 1).bits();
    let digit_cache = get_digit_cache(dec_length);
    let max_dec_cache = get_max_cache(dec_length, 10);

    (min_bin_length..=max_bin_length).for_each(|bin_length| {
        let digit_cache_ref = &digit_cache;
        let max_dec_cache_ref = &max_dec_cache;
        find_palindrome_internal(
            dec_length,
            bin_length,
            digit_cache_ref,
            max_dec_cache_ref,
            start_time,
        )
    });
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
}
