import itertools
import time

start = time.process_time()

MIN_CACHE_DIGITS = 2

def check_msb_lookup(current_num, max_dec, level, bin_length, cache):
    msb_set_bits = bin_length - (max_dec ^ current_num).bit_length()
    known_bits = min(msb_set_bits - level, len(cache) - 1)
    if known_bits < 0 or cache[known_bits] is None:
        return True

    modulo = 1 << known_bits
    current_bits = current_num >> level
    final_bits = int(bin(current_num)[known_bits + level + 1:level + 1:-1],2)
    lookup_number = (final_bits - current_bits) % modulo

    return lookup_number in cache[known_bits]

def find_palindrom_internal(current_num, bin_num, level, digits, dec_length, bin_length, digit_cache, max_dec_cache, max_bin_cache, lookup_cache):
    if (level + 1) * 2 >= dec_length:
        for digit in digits:
            new_num = current_num + digit * digit_cache[level]
            bin_str = bin(new_num)[2:]
            if bin_str == bin_str[::-1] and len(bin_str) == bin_length:
                print("%.2f: %s" % (time.process_time() - start, new_num))
        return

    max_bin_add = max_bin_cache[level]
    max_dec_add = max_dec_cache[level]

    cache = lookup_cache[level + 1]

    for digit in digits:
        new_num = current_num + digit * digit_cache[level]
        new_bin_num = bin_num + (((new_num >> level) & 1) << (bin_length - level - 1))
        new_max_dec = new_num + max_dec_add

        if new_bin_num + max_bin_add < new_num or new_max_dec < new_bin_num:
            continue

        if cache is not None and not check_msb_lookup(new_num, new_max_dec, level + 1, bin_length, cache):
            continue

        find_palindrom_internal(new_num, new_bin_num, level + 1, range(10), dec_length, bin_length, digit_cache, max_dec_cache, max_bin_cache, lookup_cache)

def get_digit_cache(dec_length):
    cache = []
    for i in range((dec_length + 1) // 2):
        j = dec_length - i - 1
        cache.append(10**i)
        if i != j:
            cache[-1] += 10**j

    return cache


def get_lookup_cache(digit_cache, preprocessing_time):
    preprocessing_start = time.process_time()
    num_digits = MIN_CACHE_DIGITS
    cache = [None] * len(digit_cache)
    while time.process_time() - preprocessing_start < preprocessing_time:
        lsb_set_bits = len(digit_cache) - num_digits

        if lsb_set_bits < 0:
            return cache

        min_log_modulo = (10**num_digits).bit_length()
        max_log_modulo = min(sum(9 * n for n in digit_cache[-num_digits:]).bit_length(), min_log_modulo + 100)

        sub_cache = [None] * min_log_modulo
        all_sums = [
            sum(multipliers[-i] * digit_cache[-i] for i in range(num_digits, 0, -1)) >> lsb_set_bits
            for multipliers in itertools.product(range(10), repeat=num_digits)
        ]

        for log_modulo in range(min_log_modulo, max_log_modulo + 1):
            modulo = 1 << log_modulo
            sub_cache.append(set(n % modulo for n in all_sums))
            if time.process_time() - preprocessing_start > preprocessing_time:
                break


        cache[lsb_set_bits] = sub_cache
        num_digits += 1

    return cache


def get_max_cache(length, base):
    return [base**(length - i) - base**i for i in range(1, (length + 1) // 2)]

def find_palindrome_lengths(dec_length, bin_length, digit_cache, lookup_cache, max_dec_cache):
    max_bin_cache = get_max_cache(bin_length, 2)
    find_palindrom_internal(0, 0, 0, range(1, 10, 2), dec_length, bin_length, digit_cache, max_dec_cache, max_bin_cache, lookup_cache)

def find_palindrome(dec_length, max_time):
    max_bin_length = (10**dec_length - 1).bit_length()
    min_bin_length = (10**(dec_length-1) + 1).bit_length()
    digit_cache = get_digit_cache(dec_length)
    max_dec_cache = get_max_cache(dec_length, 10)

    lookup_cache = get_lookup_cache(digit_cache, max_time * 0.1)

    time_before = time.process_time()

    for bin_length in range(min_bin_length, max_bin_length + 1):
        find_palindrome_lengths(dec_length, bin_length, digit_cache, lookup_cache, max_dec_cache)

    return time.process_time() - time_before

def main():
    # find_palindrome(17, 0.5)
    # get_lookup_cache(get_digit_cache(10),1)
    # exit(1)
    for i in range(10):
        if bin(i)[2:] == bin(i)[:1:-1]:
            print("%.2f: %s" % (time.process_time() - start, i))
    dec_length = 2 #13

    max_time = 0
    while True:
        search_time = find_palindrome(dec_length, max_time)
        max_time = max(search_time, max_time)
        dec_length += 1

if __name__ == '__main__':
    main()
