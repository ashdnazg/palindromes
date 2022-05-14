import time

start_time = time.process_time()

def palindrome_found(num):
    print("%08.4f: %s" % (time.process_time() - start_time, num))

def check_if_binary_palindrome(num):
    bin_str = bin(num)[2:]
    if bin_str == bin_str[::-1]:
        palindrome_found(num)

def is_pruned(decimal_digits, decimal_length):
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

    return min_binary_palindrome > max_decimal_palindrome or max_binary_palindrome < min_decimal_palindrome

def find_palindromes(current_digits, decimal_length):
    if len(current_digits) * 2 >= decimal_length:
        digits_remaining = decimal_length - len(current_digits)
        check_if_binary_palindrome(int(current_digits[:digits_remaining] + current_digits[::-1]))
        return

    if current_digits and is_pruned(current_digits, decimal_length):
        return

    if len(current_digits) == 0:
        digits = range(1, 10, 2)
    else:
        digits = range(10)

    for digit in digits:
        new_digits = current_digits + str(digit)
        find_palindromes(new_digits, decimal_length)


def main():
    decimal_length = 1
    while True:
        find_palindromes("", decimal_length)
        decimal_length += 1


if __name__ == '__main__':
    main()
