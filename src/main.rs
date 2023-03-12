#![allow(clippy::too_many_arguments)]
use ethnum::u256;
use once_cell::sync::OnceCell;
use serde::{Serialize, Deserialize};
use std::{
    sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, Mutex},
    time::Instant,
};
use sysinfo::{RefreshKind, SystemExt};

mod sort;

struct CappedScope<'scope, 'a> {
    underlying_scope: &'a rayon::ScopeFifo<'scope>,
    current_tasks: &'scope AtomicUsize,
    max_tasks: usize,
}

impl<'scope, 'a> CappedScope<'scope, 'a> {
    pub fn spawn_fifo<BODY>(&self, body: BODY) -> bool
    where
        BODY: FnOnce(&CappedScope<'scope, '_>) + Send + 'scope,
    {
        if self.current_tasks.load(Ordering::Relaxed) < self.max_tasks {
            self.force_spawn_fifo(body);

            true
        } else {
            false
        }
    }

    pub fn force_spawn_fifo<BODY>(&self, body: BODY)
    where
        BODY: FnOnce(&CappedScope<'scope, '_>) + Send + 'scope,
    {
        self.current_tasks.fetch_add(1, Ordering::Relaxed);
        let max_tasks = self.max_tasks;
        let current_tasks = self.current_tasks;
        self.underlying_scope.spawn_fifo(move |s2| {
            body(&CappedScope {
                underlying_scope: s2,
                current_tasks,
                max_tasks,
            });
            current_tasks.fetch_sub(1, Ordering::Relaxed);
        });
    }
}

trait Bits {
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

#[derive(Clone, Debug)]
struct LevelTable {
    // sorted vector of reversed suffixes of the sum of the digits after this level.
    suffixes: Vec<u64>,
    min_known_bits: u32,
    log_expanded_size: u32,
}

#[inline(never)]
fn get_digit_cache_64(digit_cache: &[[u256; 10]], level: usize) -> Vec<[u64; 10]> {
    digit_cache
        .iter()
        .skip(level)
        .map(|c| c.map(|n| *(n >> level).low() as u64))
        .collect()
}

#[inline(never)]
fn explode(instance: &mut LevelTable, sub_cache_size: u64, cancel_marker: &AtomicBool) {
    let mut i = (sub_cache_size - 1) as usize;
    while i > 0 {
        if cancel_marker.load(Ordering::Relaxed) {
            return;
        }

        let mut suffix = instance.suffixes[i];
        let mut wanted_index = (suffix >> (u64::BITS - instance.log_expanded_size)) as usize;
        loop {
            let current_contents = instance.suffixes[wanted_index];
            if current_contents <= suffix {
                break;
            }
            instance.suffixes[wanted_index] = suffix;
            suffix = current_contents;
            wanted_index += 1;
        }

        i -= 1;
    }
}

#[inline(never)]
fn populate(
    instance: &mut LevelTable,
    digit_cache_64: &Vec<[u64; 10]>,
    cancel_marker: &AtomicBool,
) {
    let mut stack: Vec<(usize, u64)> = std::iter::repeat((0, 0))
        .take(digit_cache_64.len() + 1)
        .collect();
    loop {
        if cancel_marker.load(Ordering::Relaxed) {
            return;
        }
        let sum = stack[1].1 + digit_cache_64[0][stack[0].0];
        instance.suffixes.push(sum.reverse_bits());

        let mut plus_idx = 0;
        loop {
            if stack[plus_idx].0 < 9 {
                stack[plus_idx].0 += 1;
                break;
            }
            if plus_idx == digit_cache_64.len() - 1 {
                return;
            }
            stack[plus_idx].0 = 0;
            plus_idx += 1;
        }

        while plus_idx > 0 {
            stack[plus_idx].1 = stack[plus_idx + 1].1 + digit_cache_64[plus_idx][stack[plus_idx].0];
            plus_idx -= 1;
        }
    }
}

impl LevelTable {
    fn new(
        num_digits: u32,
        digit_cache: &[[u256; 10]],
        cancel_marker: &AtomicBool,
    ) -> Option<Self> {
        let sub_cache_size = 10u64.pow(num_digits);
        let mut instance = Self {
            suffixes: Vec::with_capacity(sub_cache_size.next_power_of_two() as usize + 64),
            min_known_bits: 10u64.pow(num_digits).bits(),
            log_expanded_size: sub_cache_size.bits(),
        };

        let level = digit_cache.len() as u32 - num_digits;

        if cancel_marker.load(Ordering::Relaxed) {
            return None;
        }

        let digit_cache_64 = get_digit_cache_64(digit_cache, level as usize);

        populate(&mut instance, &digit_cache_64, cancel_marker);

        if cancel_marker.load(Ordering::Relaxed) {
            return None;
        }

        sort::quicksort(&mut instance.suffixes, u64::lt, cancel_marker);
        instance.suffixes.resize(
            instance.suffixes.capacity(),
            *instance.suffixes.last().unwrap(),
        );

        explode(&mut instance, sub_cache_size, cancel_marker);

        Some(instance)
    }

    fn lookup(&self, current_num: u256, level: u32, known_bits: u32, bin_length: u32) -> bool {
        if known_bits < self.min_known_bits {
            return true;
        }
        let known_bits = u32::min(known_bits, 64);

        let current_bits = *(current_num >> level).low() as u64;
        let shift = bin_length as i32 - level as i32 - 64;
        let final_bits = (if shift > 0 {
            *(current_num >> shift).low() as u64
        } else {
            (*current_num.low() as u64) << -shift
        })
        .reverse_bits();

        let mask = (1u64.wrapping_shl(known_bits) - 1).reverse_bits();
        let lookup_bits = final_bits.wrapping_sub(current_bits).reverse_bits() & mask;

        let guess_index = (lookup_bits >> (u64::BITS - self.log_expanded_size)) as usize;

        self.suffixes
            .iter()
            .skip(guess_index)
            .find(|&s| s & mask >= lookup_bits)
            .map_or(false, |s| s & mask == lookup_bits)
    }
}

#[derive(Clone, Debug)]
struct LookupTable {
    // index is the recursion level.
    sub_caches: Vec<OnceCell<LevelTable>>,
}

impl LookupTable {
    fn new(digit_cache: &[[u256; 10]]) -> Self {
        Self {
            sub_caches: vec![OnceCell::new(); digit_cache.len()],
        }
    }

    fn generate(
        &self,
        num_digits: u32,
        digit_cache: &[[u256; 10]],
        cancel_marker: &AtomicBool,
    ) -> bool {
        if num_digits as usize > digit_cache.len() {
            return false;
        }

        if cancel_marker.load(Ordering::Relaxed) {
            return false;
        }

        let level = digit_cache.len() - num_digits as usize;
        if let Some(level_table) = LevelTable::new(num_digits, digit_cache, cancel_marker) {
            self.sub_caches[level].set(level_table).unwrap();
        }

        !cancel_marker.load(Ordering::Relaxed)
    }

    fn lookup(&self, current_num: u256, max_dec: u256, level: u32, bin_length: u32) -> bool {
        let msb_set_bits = (bin_length as i32) - ((max_dec ^ current_num).bits() as i32);
        if (level as i32) > msb_set_bits {
            return true;
        }

        self.sub_caches[level as usize]
            .get()
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
    capped_scope: &CappedScope<'scope, '_>,
    save_state: &'scope Mutex<SaveState>,
) {
    loop {
        if TERMINATE.load(Ordering::Relaxed) {
            save_state.lock().unwrap().tasks.push(SaveTask { bin_length, stack });
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

            if !lookup_table.lookup(new_num, new_max_dec, level + 1, bin_length) {
                continue;
            }

            let msb_set_bits = (bin_length as i32) - ((new_max_dec ^ new_num).bits() as i32);

            let is_odd = if msb_set_bits <= (level as i32 + 1) {
                None
            } else {
                let wanted_digit = *(new_max_dec >> (bin_length - level - 2)).low() as u64 & 1;
                Some(*(new_num >> (level + 1)).low() as u64 & 1 != wanted_digit)
            };

            let did_spawn = capped_scope.spawn_fifo(move |capped_scope| {
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
                    capped_scope,
                    save_state
                );
            });

            if !did_spawn {
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
        let lookup_table = LookupTable::new(&digit_cache);
        let cancel = AtomicBool::new(false);

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(usize::max(
                rayon::current_num_threads(),
                (max_bin_length - min_bin_length + 2) as usize,
            ))
            .build()
            .unwrap();

        // println!(
        //     "{:.4}: Starting decimal length: {}",
        //     start_time.elapsed().as_secs_f32(),
        //     dec_length
        // );

        let max_bin_caches: Vec<_> = (min_bin_length..=max_bin_length)
            .map(|bin_length| get_max_cache(bin_length, 2))
            .collect();

        let current_tasks = AtomicUsize::new(0);
        pool.scope_fifo(|scope| {
            let capped_scope = CappedScope {
                underlying_scope: scope,
                current_tasks: &current_tasks,
                max_tasks: 8192,
            };
            let available_memory =
                sysinfo::System::new_with_specifics(RefreshKind::new().with_memory())
                    .available_memory();
            let max_cache_digits = (available_memory / std::mem::size_of::<u64>() as u64).ilog10();
            for num_digits in 2..=max_cache_digits {
                let digit_cache_ref = &digit_cache;
                let cancel_ref = &cancel;
                let lookup_table_ref = &lookup_table;
                capped_scope.force_spawn_fifo(move |_| {
                    if lookup_table_ref.generate(num_digits, digit_cache_ref, cancel_ref) {
                        // println!(
                        //     "{:.4}: Generated table for decimal length {}, num_digits: {}",
                        //     start_time.elapsed().as_secs_f32(),
                        //     dec_length,
                        //     num_digits
                        // );
                    }
                })
            }

            pool.scope_fifo(|scope| {
                let capped_scope = CappedScope {
                    underlying_scope: scope,
                    current_tasks: &current_tasks,
                    max_tasks: 8192,
                };

                let existing_tasks = &mut save_state.lock().unwrap().tasks;
                let tasks: Vec<SaveTask> = if existing_tasks.is_empty() {
                    (min_bin_length..=max_bin_length).map(|bin_length| SaveTask { stack: vec![State {
                        current_num: u256::ZERO,
                        bin_num: u256::ZERO,
                        is_odd: Some(true),
                        level: 0,
                    }], bin_length }).collect()
                } else {
                    existing_tasks.drain(..).collect()
                };

                for task in tasks {
                    let bin_length = task.bin_length;
                    let digit_cache_ref = &digit_cache;
                    let max_dec_cache_ref = &max_dec_cache;
                    let lookup_table_ref = &lookup_table;
                    let max_bin_cache_ref = &max_bin_caches[(bin_length - min_bin_length) as usize];
                    capped_scope.spawn_fifo(move |capped_scope| {
                        find_palindrome_recursive(
                            task.stack,
                            dec_length,
                            bin_length,
                            digit_cache_ref,
                            max_dec_cache_ref,
                            max_bin_cache_ref,
                            lookup_table_ref,
                            start_time,
                            capped_scope,
                            save_state
                        );
                    });
                }
            });
            // println!(
            //     "{:.4}: Finished decimal length {}",
            //     start_time.elapsed().as_secs_f32(),
            //     dec_length
            // );
            cancel.store(true, Ordering::Relaxed);
        });

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
    stack: Vec<State>
}

#[derive(Serialize, Deserialize)]
struct SaveState {
    dec_length: u32,
    tasks: Vec<SaveTask>,
    palindromes_found: Vec<u256>,
}

fn main() {
    let save_path_arg = std::env::args().skip(1).next();
    let mut save_state = Mutex::new(SaveState { dec_length: 1, tasks: vec![], palindromes_found: vec![] });
    if let Some(save_path) = &save_path_arg {
        let load_result = std::fs::read_to_string(&save_path);
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
}
