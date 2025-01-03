#![allow(clippy::too_many_arguments)]
use ethnum::u256;
use rayon::Scope;
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Instant,
};
use sysinfo::{MemoryRefreshKind, RefreshKind};

mod par_bitmap_table;

use par_bitmap_table::{LevelTable, LookupTable};

const VERBOSE: bool = false;

pub trait Bits {
    fn bits(&self) -> u32;
}

impl Bits for u256 {
    fn bits(&self) -> u32 {
        Self::BITS - self.leading_zeros()
    }
}

impl Bits for u64 {
    fn bits(&self) -> u32 {
        Self::BITS - self.leading_zeros()
    }
}

impl Bits for usize {
    fn bits(&self) -> u32 {
        Self::BITS - self.leading_zeros()
    }
}

#[derive(Serialize, Deserialize)]
struct State {
    current_num: u256,
    bin_num: u256,
    is_odd: Option<bool>,
    level: u32,
}

fn find_palindrome_recursive<'scope>(
    mut stack: Vec<State>,
    dec_length: u32,
    bin_length: u32,
    digit_cache: &'scope [[u256; 10]],
    max_dec_cache: &'scope [u256],
    max_bin_cache: &'scope [u256],
    lookup_table: &'scope LookupTable,
    start_time: Instant,
    scope: &Scope<'scope>,
    save_state: &'scope Mutex<SaveState>,
) {
    loop {
        if TERMINATE.load(Ordering::Relaxed) {
            save_state
                .lock()
                .unwrap()
                .tasks
                .push(SaveTask { bin_length, stack });
            return;
        }
        let Some(state) = stack.pop() else { return };

        let current_num = state.current_num;
        let bin_num = state.bin_num;
        let level = state.level;

        let digits = match state.is_odd {
            Some(true) => (1..=9).step_by(2),
            Some(false) => (0..=8).step_by(2),
            None => (0..=9).step_by(1),
        };

        if (state.level + 1) * 2 >= dec_length {
            for digit in digits {
                let new_num = state.current_num + digit_cache[level as usize][digit as usize];
                let leading_zeros = new_num.leading_zeros();
                if leading_zeros + bin_length != u256::BITS {
                    continue;
                }

                let reversed = new_num.reverse_bits() >> leading_zeros;
                if reversed == new_num {
                    println!("{:.4}: {}", start_time.elapsed().as_secs_f32(), new_num);
                    save_state.lock().unwrap().palindromes_found.push(new_num);
                }
            }

            continue;
        }

        let max_bin_add = max_bin_cache[level as usize];
        let max_dec_add = max_dec_cache[level as usize];

        for digit in digits {
            let new_num = current_num + digit_cache[level as usize][digit as usize];
            let new_bin_num = bin_num + (((new_num >> level) & 1) << (bin_length - level - 1));

            let new_max_dec = new_num + max_dec_add;

            if new_bin_num + max_bin_add < new_num || new_max_dec < new_bin_num {
                continue;
            }

            let msb_set_bits = (bin_length as i32) - ((new_max_dec ^ new_num).bits() as i32);

            if !lookup_table.lookup(new_num, msb_set_bits, level + 1, bin_length) {
                continue;
            }

            let is_odd = if msb_set_bits <= (level as i32 + 1) {
                None
            } else {
                let wanted_digit = *(new_max_dec >> (bin_length - level - 2)).low() as u64 & 1;
                Some(*(new_num >> (level + 1)).low() as u64 & 1 != wanted_digit)
            };

            if level < 4 {
                scope.spawn(move |scope| {
                    find_palindrome_recursive(
                        vec![State {
                            current_num: new_num,
                            bin_num: new_bin_num,
                            is_odd,
                            level: level + 1,
                        }],
                        dec_length,
                        bin_length,
                        digit_cache,
                        max_dec_cache,
                        max_bin_cache,
                        lookup_table,
                        start_time,
                        scope,
                        save_state,
                    );
                })
            } else {
                stack.push(State {
                    current_num: new_num,
                    bin_num: new_bin_num,
                    is_odd,
                    level: level + 1,
                });
            }
        }
    }
}

fn find_palindrome(save_state: &Mutex<SaveState>, start_time: Instant) {
    loop {
        let dec_length = save_state.lock().unwrap().dec_length;
        let max_bin_length = (u256::from(10u32).pow(dec_length) - 1).bits();
        let min_bin_length = (u256::from(10u32).pow(dec_length - 1) + 1).bits();
        let digit_cache = get_digit_cache(dec_length);
        let max_dec_cache = get_max_cache(dec_length, 10);
        let mut lookup_table = LookupTable::new(&digit_cache);

        if VERBOSE {
            println!(
                "{:.4}: Starting decimal length: {}",
                start_time.elapsed().as_secs_f32(),
                dec_length
            );
        }

        let max_bin_caches: Vec<_> = (min_bin_length..=max_bin_length)
            .map(|bin_length| get_max_cache(bin_length, 2))
            .collect();

        let mut remaining_memory = sysinfo::System::new_with_specifics(
            RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
        )
        .available_memory();
        // println!("available memory: {:?}", remaining_memory);
        let desired_max_cache_digits =
            (dec_length as f64 * 5f64.log2() / (2f64 * 5f64.log2() + 1f64) / 2f64).floor() as u32;
        let max_cache_digits = (remaining_memory * 8)
            .ilog10()
            .min(desired_max_cache_digits);
        // let max_cache_digits = 11; //(available_memory / std::mem::size_of::<u64>() as u64).ilog10();
        if VERBOSE {
            println!("max_cache_digits: {desired_max_cache_digits}");
        }
        for num_digits in (2..=max_cache_digits).rev() {
            let Some((downscale_factor, size)) =
                LevelTable::calculate_memory_requirements(num_digits, remaining_memory)
            else {
                continue;
            };
            remaining_memory -= size;
            if VERBOSE {
                println!(
                    "Generating table for decimal length {}, num_digits: {}",
                    dec_length, num_digits
                );
            }
            if lookup_table.generate(num_digits, downscale_factor, &digit_cache) {
                let level = digit_cache.len() - num_digits as usize;
                let instance = lookup_table.sub_caches[level].as_ref().unwrap();
                if VERBOSE {
                    println!(
                        "{:.4}: Generated table for decimal length {}, num_digits: {}, size: {}, factor: {}",
                        start_time.elapsed().as_secs_f32(),
                        dec_length,
                        num_digits,
                        instance.size(),
                        10u64.pow(num_digits) as f64 / (instance.size() * 8) as f64
                    );
                }
            }
        }

        rayon::scope(|scope| {
            let existing_tasks = &mut save_state.lock().unwrap().tasks;
            let tasks: Vec<SaveTask> = if existing_tasks.is_empty() {
                (min_bin_length..=max_bin_length)
                    .map(|bin_length| SaveTask {
                        stack: vec![State {
                            current_num: u256::ZERO,
                            bin_num: u256::ZERO,
                            is_odd: Some(true),
                            level: 0,
                        }],
                        bin_length,
                    })
                    .collect()
            } else {
                std::mem::take(existing_tasks)
            };

            for task in tasks {
                let bin_length = task.bin_length;
                let digit_cache_ref = &digit_cache;
                let max_dec_cache_ref = &max_dec_cache;
                let lookup_table_ref = &lookup_table;
                let max_bin_cache_ref = &max_bin_caches[(bin_length - min_bin_length) as usize];
                scope.spawn(move |scope| {
                    find_palindrome_recursive(
                        task.stack,
                        dec_length,
                        bin_length,
                        digit_cache_ref,
                        max_dec_cache_ref,
                        max_bin_cache_ref,
                        lookup_table_ref,
                        start_time,
                        scope,
                        save_state,
                    );
                });
            }
        });
        if VERBOSE {
            println!(
                "{:.4}: Finished decimal length {}",
                start_time.elapsed().as_secs_f32(),
                dec_length
            );
        }

        if save_state.lock().unwrap().tasks.is_empty() {
            save_state.lock().unwrap().dec_length += 1;
        } else {
            return;
        }
    }
}

fn get_max_cache(length: u32, base: u32) -> Vec<u256> {
    let cache_length = (length + 1) / 2;
    (1..cache_length)
        .map(|i| u256::from(base).pow(length - i) - u256::from(base).pow(i))
        .collect()
}

fn get_digit_cache(dec_length: u32) -> Vec<[u256; 10]> {
    let cache_length = (dec_length + 1) / 2;
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

static TERMINATE: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Deserialize)]
struct SaveTask {
    bin_length: u32,
    stack: Vec<State>,
}

#[derive(Serialize, Deserialize)]
struct SaveState {
    dec_length: u32,
    tasks: Vec<SaveTask>,
    palindromes_found: Vec<u256>,
}

fn main() {
    let save_path_arg = std::env::args().nth(1);
    let mut save_state = Mutex::new(SaveState {
        dec_length: 1,
        tasks: vec![],
        palindromes_found: vec![],
    });
    if let Some(save_path) = &save_path_arg {
        let load_result = std::fs::read_to_string(save_path);
        if let Ok(contents) = load_result {
            *save_state.get_mut().unwrap() = serde_json::from_str(&contents).unwrap()
        }

        ctrlc::set_handler(move || TERMINATE.store(true, Ordering::Relaxed))
            .expect("Error setting Ctrl-C handler");
    }
    let start_time = Instant::now();
    find_palindrome(&save_state, start_time);
    if let Some(save_path) = &save_path_arg {
        let serialized_save_state = serde_json::to_string(&*save_state.lock().unwrap()).unwrap();
        std::fs::write(save_path, serialized_save_state).unwrap();
    }

    // table_tests();
}

#[allow(dead_code)]
fn table_tests() {
    let digit_cache = get_digit_cache(46);
    let mut lookup = par_bitmap_table::LookupTable::new(&digit_cache);
    let start_time = Instant::now();
    let num_digits = 10;
    lookup.generate(num_digits, 3, &digit_cache);
    let level = digit_cache.len() - num_digits as usize;
    println!("{:.4}: Finished bitmap", start_time.elapsed().as_secs_f32());
    let sat = lookup.sub_caches[level].as_ref().unwrap().saturation();
    println!(
        "{:.4}: Bitmap saturation: {sat}",
        start_time.elapsed().as_secs_f32()
    );
}
