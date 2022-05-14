import sys
import matplotlib.pyplot as plt
import numpy as np
from scipy.optimize import curve_fit

def linear_law(x, a, b):
    return a * x + b

def power_law(x, a, b):
    return b * np.power(x, a)

def rev_power_law(y, a, b):
    return np.power(y / b, 1.0 / a)

def get_file_data(i, file_path, n):
    times = []
    palindromes = []
    with open(file_path, "r") as results_file:
        for line in results_file:
            line = line.strip()
            if not line:
                continue

            time_str, palindrome_str = line.split(": ")
            times.append(float(time_str))
            palindromes.append(float(palindrome_str))
    times = np.array(times)
    palindromes = np.array(palindromes)
    first_index = np.argmax(times >= 1.0)
    last_index = np.argmax(times > 600.0) if np.any(times > 600.0) else len(times)
    times = times[first_index:last_index]
    palindromes = palindromes[first_index:last_index]
    color = ["green", "blue", "red", "purple", "teal", "brown"][i]

    popt, _ = curve_fit(linear_law, np.log(times), np.log(palindromes))
    a, b = popt[0], np.exp(popt[1])

    print(a, b)
    hours = rev_power_law(9335388324586156026843333486206516854238835339, a, b) / 3600.0
    days = hours / 24.0
    years = days / 365.0
    months = years * 12.0
    print("%.2f hours = %.2f days = %.2f months = %.2g years" % (hours, days, months, years))

    x = np.arange(1, 600, 1)
    plt.loglog(x, power_law(x, a, b), '--', color=color)
    plt.loglog(times, palindromes, '.', color=color)
    exp10 = np.round(np.log10(b))
    text_y = power_law(600, a, b)
    if n >= 3 and (i == 1 or i == 2):
        correction = (i - 1.5) * 5
        text_y = np.exp(np.log(text_y) + correction)
    plt.text(650, text_y, "$p=10^{\,%d} \\times t^{\,%.2f}$"%(exp10, a), color=color, fontsize=12)
    plt.subplots_adjust(right=0.75)

    return (times, palindromes)

def main(result_paths):
    for i, path in enumerate(result_paths):
        times, palindromes = get_file_data(i, path, len(result_paths))

    plt.xlim(1, 600)
    plt.ylim(1, plt.ylim()[1])
    plt.xlabel("Time (s)", fontsize=12)
    plt.ylabel("Palindrome", fontsize=12)
    plt.title("Time to find palindromes", fontsize=12)
    plt.show()




if __name__ == '__main__':
    main(sys.argv[1:])
