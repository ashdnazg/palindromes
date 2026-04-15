use ethnum::u256;


pub struct RemainderTable {
    tables: Vec<Vec<u8>>
}

fn create_level(binary_length: u32, divisor: u64) -> Vec<u8> {
    let half_length = (binary_length + 1) / 2;
    let mods: Vec<_> = (0..half_length)
        .map(|i| {
            let j = binary_length - i - 1;
            let mut entry = u256::from(2u32).pow(i);
            if i != j {
                entry += u256::from(2u32).pow(j);
            }

            *(entry % (divisor as u128)).low() as u64
        })
        .collect();

    let mut ret = vec![255;divisor as usize];
    let mut count = 1;
    ret[0] = 0;
    for (i, remainder) in mods.iter().enumerate() {
        for j in 0..divisor {
            if ret[j as usize] >= i as u8 + 1 {
                continue;
            }
            let new_remainder = ((j + remainder) % divisor) as usize;
            if ret[new_remainder as usize] == 255 {
                ret[new_remainder] = i as u8 + 1;
                count += 1;
            }
        }
        if count == divisor {
            break;
        }
    }

    ret
}

impl RemainderTable {
    pub fn new(binary_length: u32, max_digit_count: u32) -> Self {
        let multiplier = if binary_length & 1 == 0 {
            11
        } else {
            1
        };

        let mut tables = Vec::with_capacity(max_digit_count as usize);

        for i in 0..=max_digit_count {
            let level = create_level(binary_length, 5u64.pow(i) * multiplier);
            tables.push(level);
        }

        RemainderTable { tables }
    }

    pub fn lookup(&self, current_num: u256, level: u32, known_bits: u32, binary_length: u32) -> bool {
        let unknown_bits = binary_length - known_bits * 2;
        let level_table = &self.tables[(level as usize).min(self.tables.len() - 1)];
        let divisor = level_table.len();
        if 1 << unknown_bits > divisor {
            return true;
        }
        let shifted = current_num >> (binary_length - known_bits);
        let shifted_back = shifted << (binary_length - known_bits);
        let reversed = shifted.reverse_bits();
        let reversed_shifted = reversed >> (u256::BITS - known_bits);
        let known_binary_num = shifted_back | reversed_shifted;
        let subtracted = current_num - known_binary_num;
        let remainder = *(subtracted % divisor as u128).low() as usize;

        level_table[remainder] as u32 <= unknown_bits
    }
}
