import time

start_time = time.process_time()

def palindrome_found(num):
    print("%08.4f: %s" % (time.process_time() - start_time, num))

def check_if_palindrome(num):
    bin_str = bin(num)[2:]
    dec_str = str(num)
    if bin_str == bin_str[::-1] and dec_str == dec_str[::-1]:
        palindrome_found(num)

def main():
    i = 1
    while True:
        check_if_palindrome(i)
        i += 1


if __name__ == '__main__':
    main()
