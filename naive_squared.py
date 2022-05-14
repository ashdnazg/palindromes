import time

start_time = time.process_time()

def palindrome_found(num):
    print("%08.4f: %s" % (time.process_time() - start_time, num))

def check_if_binary_palindrome(num):
    bin_str = bin(num)[2:]
    if bin_str == bin_str[::-1]:
        palindrome_found(num)

def main():
    i = 1
    while True:
        check_if_binary_palindrome(int(str(i) + str(i)[::-1]))
        check_if_binary_palindrome(int(str(i)[:-1] + str(i)[::-1]))
        i += 1


if __name__ == '__main__':
    main()
