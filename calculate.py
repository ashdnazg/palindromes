import time

start = time.time()

MIN_CACHE3_SIZE = 10
MAX_CACHE3_SIZE = 25

MIN_CACHE4_SIZE = 12
MAX_CACHE4_SIZE = 23

def check_msb_lookup(current_num, max_dec_add, level, dec_length, bin_length, lookup_cache):
    if level + 4 == ((dec_length + 1) // 2):
        cache = lookup_cache[1]
        min_cache_size = MIN_CACHE4_SIZE
        max_cache_size = MAX_CACHE4_SIZE
    elif level + 3 == ((dec_length + 1) // 2):
        cache = lookup_cache[0]
        min_cache_size = MIN_CACHE3_SIZE
        max_cache_size = MAX_CACHE3_SIZE
    else:
        return True

    max_dec = current_num + max_dec_add
    msb_set_digits = bin_length - len(bin(max_dec ^ current_num)) + 2
    if msb_set_digits < (level + min_cache_size):
        return True
    cache_digits = min(msb_set_digits - level, max_cache_size)
    modulo = 1 << cache_digits
    current_digits = (current_num >> level) % modulo
    final_bin_digits = (int(bin(max_dec)[msb_set_digits + 1:1:-1],2) >> level)
    lookup_number = ((final_bin_digits + modulo) - current_digits) % modulo
    return lookup_number in cache[cache_digits]

def find_palindrom_internal(current_num, bin_num, level, digits, dec_length, bin_length, digit_cache, max_dec_cache, max_bin_cache, lookup_cache):
    if (level + 1) * 2 >= dec_length:
        for digit in digits:
            new_num = current_num + digit * digit_cache[level]
            bin_str = bin(new_num)[2:]
            if bin_str == bin_str[::-1] and len(bin_str) == bin_length:
                print("%.2f: %s" % (time.time() - start, new_num))
        return

    max_bin_add = max_bin_cache[level]
    max_dec_add = max_dec_cache[level]

    for digit in digits:
        new_num = current_num + digit * digit_cache[level]
        new_bin_num = bin_num + (((new_num >> level) & 1) << (bin_length - level - 1))

        if new_bin_num + max_bin_add < new_num or new_num + max_dec_add < new_bin_num:
            continue

        if not check_msb_lookup(new_num, max_dec_add, level + 1, dec_length, bin_length, lookup_cache):
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


def get_lookup_cache(digit_cache):
    if len(digit_cache) < 3:
        return

    cache3 = {}
    lsb_set_bits = len(digit_cache) - 3
    for cache_size in range(MIN_CACHE3_SIZE, MAX_CACHE3_SIZE + 1):
        current_cache = set()
        modulo = 1 << cache_size
        for i in range(10):
            for j in range(10):
                for k in range(10):
                    digits_sum = digit_cache[-1] * i + digit_cache[-2] * j + digit_cache[-3] * k
                    current_cache.add((digits_sum >> lsb_set_bits) % modulo)

        cache3[cache_size] = current_cache

    if len(digit_cache) < 4:
        return

    cache4 = {}
    lsb_set_bits = len(digit_cache) - 4
    for cache_size in range(MIN_CACHE4_SIZE, MAX_CACHE4_SIZE + 1):
        current_cache = set()
        modulo = 1 << cache_size
        for i in range(10):
            for j in range(10):
                for k in range(10):
                    for w in range(10):
                        digits_sum = digit_cache[-1] * i + digit_cache[-2] * j + digit_cache[-3] * k + digit_cache[-4] * w
                        current_cache.add((digits_sum >> lsb_set_bits) % modulo)

        cache4[cache_size] = current_cache

    return (cache3, cache4)


def get_max_cache(length, base):
    return [base**(length - i) - base**i for i in range(1, (length + 1) // 2)]

def find_palindrome(dec_length, bin_length):
    digit_cache = get_digit_cache(dec_length)
    lookup_cache = get_lookup_cache(digit_cache)
    max_dec_cache = get_max_cache(dec_length, 10)
    max_bin_cache = get_max_cache(bin_length, 2)
    find_palindrom_internal(0, 0, 0, range(1, 10, 2), dec_length, bin_length, digit_cache, max_dec_cache, max_bin_cache, lookup_cache)

def is_possible(dec_length, bin_length):
    max_dec = int("9" * dec_length)
    min_dec = int("1" + ("0" * (dec_length - 2)) + "1")
    max_bin = int("1" * bin_length, 2)
    min_bin = int("1" + ("0" * (bin_length - 2)) + "1", 2)
    return min_bin <= max_dec and max_bin >= min_dec

def main():
    # find_palindrome(29, 94)
    # exit(1)
    for i in range(10):
        if bin(i)[2:] == bin(i)[:1:-1]:
            print("%.2f: %s" % (time.time() - start, i))
    dec_length = 2 #13
    bin_length = 4 #41

    while True:
        find_palindrome(dec_length, bin_length)
        if is_possible(dec_length, bin_length + 1):
            bin_length += 1
        else:
            dec_length += 1

if __name__ == '__main__':
    main()