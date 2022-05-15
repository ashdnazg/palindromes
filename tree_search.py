import time

start_time = time.process_time()

def palindrome_found(num):
    print("%08.4f: %s" % (time.process_time() - start_time, num))

def check_if_binary_palindrome(num):
    bin_str = bin(num)[2:]
    if bin_str == bin_str[::-1]:
        palindrome_found(num)

def find_palindromes(current_digits, decimal_length):
    if len(current_digits) * 2 >= decimal_length:
        digits_remaining = decimal_length - len(current_digits)
        check_if_binary_palindrome(int(current_digits[:digits_remaining] + current_digits[::-1]))
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
