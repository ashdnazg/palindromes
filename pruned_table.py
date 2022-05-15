import itertools
import time

start_time = time.process_time()

def palindrome_found(num):
    print("%08.4f: %s" % (time.process_time() - start_time, num))

def check_if_binary_palindrome(num):
    bin_str = bin(num)[2:]
    if bin_str == bin_str[::-1]:
        palindrome_found(num)

def is_in_table(subtraction_suffix, shared_bits_length, lookup_table):
    table, log_expanded_size, current_digits_length = lookup_table
    lookup_bits_length = shared_bits_length - current_digits_length
    if lookup_bits_length <= 0:
        return True

    mask = ((1 << lookup_bits_length) - 1) << (64 - lookup_bits_length)
    lookup_number = int(bin(subtraction_suffix >> current_digits_length)[:1:-1].ljust(64, "0"), 2) & mask
    guess_index = lookup_number >> (64 - log_expanded_size)
    while guess_index < len(table):
        value = table[guess_index]
        if value >= lookup_number:
            return value & mask == lookup_number

        guess_index += 1

    return False



def is_pruned_by_table(min_decimal_palindrome, max_decimal_palindrome, lookup_table):
    nonshared_bits_length = (max_decimal_palindrome ^ min_decimal_palindrome).bit_length()
    shared_most_significant_bits = bin(min_decimal_palindrome >> nonshared_bits_length)[2:]
    hypothetical_least_significant_bits = int(shared_most_significant_bits[::-1], 2)
    subtraction_result = hypothetical_least_significant_bits - min_decimal_palindrome
    subtraction_least_significant_bits = subtraction_result % (1 << len(shared_most_significant_bits))

    return not is_in_table(subtraction_least_significant_bits, len(shared_most_significant_bits), lookup_table)

def is_pruned(decimal_digits, decimal_length, lookup_table_array):
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

    lookup_table = lookup_table_array[len(decimal_digits)]

    return lookup_table and is_pruned_by_table(min_decimal_palindrome, max_decimal_palindrome, lookup_table)

def find_palindromes(current_digits, decimal_length, lookup_table_array):
    if len(current_digits) * 2 >= decimal_length:
        digits_remaining = decimal_length - len(current_digits)
        check_if_binary_palindrome(int(current_digits[:digits_remaining] + current_digits[::-1]))
        return

    if current_digits and is_pruned(current_digits, decimal_length, lookup_table_array):
        return

    if len(current_digits) == 0:
        digits = range(1, 10, 2)
    else:
        digits = range(10)

    for digit in digits:
        new_digits = current_digits + str(digit)
        find_palindromes(new_digits, decimal_length, lookup_table_array)

MIN_TABLE_DIGITS = 2

def create_table_array(decimal_length, max_table_digits):
    digit_cache = []
    for i in range((decimal_length + 1) // 2):
        j = decimal_length - i - 1
        digit_cache.append(10**i)
        if i != j:
            digit_cache[-1] += 10**j

    max_table_digits = min(len(digit_cache), max_table_digits)

    table_array = [None] * (len(digit_cache) + 1)
    for digits_remaining in range(MIN_TABLE_DIGITS, max_table_digits + 1):
        current_digits_length = len(digit_cache) - digits_remaining
        suffixes = [
            int(bin(sum(multipliers[-i] * digit_cache[-i] for i in range(digits_remaining, 0, -1)) >> current_digits_length)[:1:-1].ljust(64, "0"), 2)
            for multipliers in itertools.product(range(10), repeat=digits_remaining)
        ]
        suffixes.sort()
        log_expanded_size = len(suffixes).bit_length()
        expanded_size = 1 << log_expanded_size
        table = []
        for suffix in suffixes:
            wanted_index = suffix >> (64 - log_expanded_size)
            while True:
                table.append(suffix)
                if wanted_index < len(table):
                    break

        table_array[current_digits_length] = (table, log_expanded_size, current_digits_length)

    return table_array

def main():
    # Blatant cheating, created using pruned_profile.py
    best_max_table_digits = [1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6]
    decimal_length = 1
    while True:
        max_table_digits = best_max_table_digits[min(decimal_length, len(best_max_table_digits) - 1)]
        lookup_table_array = create_table_array(decimal_length, max_table_digits)
        find_palindromes("", decimal_length, lookup_table_array)
        decimal_length += 1


if __name__ == '__main__':
    main()
