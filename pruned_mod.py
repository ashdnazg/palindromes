import itertools
import time

start_time = time.process_time()

def palindrome_found(num):
    print("%08.4f: %s" % (time.process_time() - start_time, num))

def check_if_binary_palindrome(num):
    bin_str = bin(num)[2:]
    if bin_str == bin_str[::-1]:
        palindrome_found(num)

TABLE_LEVEL = 8

MOD = 5**TABLE_LEVEL

def is_pruned_by_table(min_decimal_palindrome, max_decimal_palindrome, binary_length, known_digits, lookup_table_dict):
    if known_digits < TABLE_LEVEL:
        return False

    nonshared_bits_length = (max_decimal_palindrome ^ min_decimal_palindrome).bit_length()
    unknown_bits = nonshared_bits_length - binary_length // 2

    if binary_length not in lookup_table_dict:
        lookup_table_dict[binary_length] = populate_remainder_table(binary_length, MOD)

    lookup_table = lookup_table_dict[binary_length]

    shifted = min_decimal_palindrome >> nonshared_bits_length
    shared_most_significant_bits = bin(shifted)[2:]
    hypothetical_least_significant_bits = int(shared_most_significant_bits[::-1], 2)
    known_bits = shifted << nonshared_bits_length | hypothetical_least_significant_bits

    subtraction_result = min_decimal_palindrome - known_bits
    mod = subtraction_result % MOD

    return lookup_table[mod] > unknown_bits

def is_pruned(decimal_digits, decimal_length, lookup_table_dict):
    changing_decimal_length = decimal_length - 2 * len(decimal_digits)
    min_decimal_palindrome = int(decimal_digits + "0" * changing_decimal_length + decimal_digits[::-1])
    max_decimal_palindrome = int(decimal_digits + "9" * changing_decimal_length + decimal_digits[::-1])

    binary_length = min_decimal_palindrome.bit_length()
    if max_decimal_palindrome.bit_length() != binary_length:
        return False

    binary_digits = bin(min_decimal_palindrome)[-len(decimal_digits):]

    changing_binary_length = binary_length - 2 * len(binary_digits)
    min_binary_palindrome = int(binary_digits[::-1] + "0" * changing_binary_length + binary_digits, 2)
    max_binary_palindrome = int(binary_digits[::-1] + "1" * changing_binary_length + binary_digits, 2)

    if min_binary_palindrome > max_decimal_palindrome or max_binary_palindrome < min_decimal_palindrome:
        return True

    return is_pruned_by_table(min_decimal_palindrome, max_decimal_palindrome, binary_length, len(decimal_digits), lookup_table_dict)

def find_palindromes(current_digits, decimal_length, lookup_table_dict):
    if len(current_digits) * 2 >= decimal_length:
        digits_remaining = decimal_length - len(current_digits)
        check_if_binary_palindrome(int(current_digits[:digits_remaining] + current_digits[::-1]))
        return

    if current_digits and is_pruned(current_digits, decimal_length, lookup_table_dict):
        return

    if len(current_digits) == 0:
        digits = range(1, 10, 2)
    else:
        digits = range(10)

    for digit in digits:
        new_digits = current_digits + str(digit)
        find_palindromes(new_digits, decimal_length, lookup_table_dict)

def populate_remainder_table(bin_length, mod):
    mods = []
    for i in range((bin_length + 1) // 2):
        idx1 = bin_length - 1 - i
        idx2 = i
        if idx1 == idx2:
            mod_value = (1 << idx1) % mod
        else:
            mod_value = ((1 << idx1) + (1 << idx2)) % mod
        mods.append(mod_value)

    ret = [255] * mod
    count = 1
    ret[0] = 0
    for i, mod_value in enumerate(mods[::-1]):
        for j in range(mod):
            if ret[j] < i + 1:
                new_mod = (j + mod_value) % mod
                if ret[new_mod] == 255:
                    ret[new_mod] = i + 1
                    count += 1
        if count == mod:
            break

    return ret

def main():
    decimal_length = 1
    while True:
        lookup_table_dict = {}
        find_palindromes("", decimal_length, lookup_table_dict)
        decimal_length += 1


if __name__ == '__main__':
    main()
