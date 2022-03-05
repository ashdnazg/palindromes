import time

start = time.time()

def find_palindrom_internal(current_num, bin_num, level, digits, dec_length, bin_length, digit_cache, max_dec_cache, max_bin_cache):
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

        find_palindrom_internal(new_num, new_bin_num, level + 1, range(10), dec_length, bin_length, digit_cache, max_dec_cache, max_bin_cache)

def get_digit_cache(dec_length):
    cache = []
    for i in range((dec_length + 1) // 2):
        j = dec_length - i - 1
        cache.append(10**i)
        if i != j:
            cache[-1] += 10**j

    return cache

def get_max_cache(length, base):
    return [base**(length - i) - base**i for i in range(1, (length + 1) // 2)]

def find_palindrome(dec_length, bin_length):
    digit_cache = get_digit_cache(dec_length)
    max_dec_cache = get_max_cache(dec_length, 10)
    max_bin_cache = get_max_cache(bin_length, 2)
    find_palindrom_internal(0, 0, 0, range(1, 10, 2), dec_length, bin_length, digit_cache, max_dec_cache, max_bin_cache)

def is_possible(dec_length, bin_length):
    max_dec = int("9" * dec_length)
    min_dec = int("1" + ("0" * (dec_length - 2)) + "1")
    max_bin = int("1" * bin_length, 2)
    min_bin = int("1" + ("0" * (bin_length - 2)) + "1", 2)
    return min_bin <= max_dec and max_bin >= min_dec

def main():
    for i in range(10):
        if bin(i)[2:] == bin(i)[:1:-1]:
            print("%.2f: %s" % (time.time() - start, i))
    dec_length = 2
    bin_length = 4

    while True:
        find_palindrome(dec_length, bin_length)
        if is_possible(dec_length, bin_length + 1):
            bin_length += 1
        else:
            dec_length += 1

if __name__ == '__main__':
    main()