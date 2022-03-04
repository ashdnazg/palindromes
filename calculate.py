def is_consistent(current_digits, dec_length, bin_length):
    dec_digits_left = dec_length - len(current_digits) * 2
    max_dec = int(current_digits + ("9" * dec_digits_left) + current_digits[::-1])
    min_dec = int(current_digits + ("0" * dec_digits_left) + current_digits[::-1])
    bin_digits = bin(int(current_digits[::-1]) % (1 << len(current_digits)))[2:].zfill(len(current_digits))
    bin_digits_left = bin_length - len(current_digits) * 2
    max_bin = int(bin_digits[::-1] + ("1" * bin_digits_left) + bin_digits, 2)
    min_bin = int(bin_digits[::-1] + ("0" * bin_digits_left) + bin_digits, 2)
    return min_bin <= max_dec and max_bin >= min_dec

def digits_to_num(current_digits, dec_length):
    return int(current_digits + current_digits[dec_length - len(current_digits) - 1::-1])

def check_palindrome(current_digits, dec_length, bin_length):
    bin_num = bin(digits_to_num(current_digits, dec_length))[2:]
    return len(bin_num) == bin_length and bin_num == bin_num[::-1]

def find_palindrom_internal(current_digits, dec_length, bin_length):
    if len(current_digits) == 0:
        digits = range(1, 10, 2)
    else:
        digits = range(10)

    for digit in digits:
        new_digits = current_digits + str(digit)

        if len(new_digits) * 2 >= dec_length:
            if check_palindrome(new_digits, dec_length, bin_length):
                print(digits_to_num(new_digits, dec_length))
            continue

        if not is_consistent(new_digits, dec_length, bin_length):
            continue

        find_palindrom_internal(new_digits, dec_length, bin_length)

def find_palindrome(dec_length, bin_length):
    find_palindrom_internal("", dec_length, bin_length)

def is_possible(dec_length, bin_length):
    max_dec = int("9" * dec_length)
    min_dec = int("1" + ("0" * (dec_length - 2)) + "1")
    max_bin = int("1" * bin_length, 2)
    min_bin = int("1" + ("0" * (bin_length - 2)) + "1", 2)
    return min_bin <= max_dec and max_bin >= min_dec

def main():
    for i in range(10):
        if bin(i)[2:] == bin(i)[:1:-1]:
            print(i)
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