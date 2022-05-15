import matplotlib.pyplot as plt
import matplotlib.offsetbox
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
    labels = ["Naive", "Mirror", "Tree Search", "Range Pruning", "Range+Table Pruning", "Range+Table Pruning - Rust"]

    popt, _ = curve_fit(linear_law, np.log(times), np.log(palindromes))
    a, b = popt[0], np.exp(popt[1])

    print(a, b)
    hours = rev_power_law(9335388324586156026843333486206516854238835339, a, b) / 3600.0
    days = hours / 24.0
    years = days / 365.0
    months = years * 12.0
    print("%.2f hours = %.2f days = %.2f months = %.2f years" % (hours, days, months, years))

    x = np.arange(1, 600, 1)
    plt.loglog(x, power_law(x, a, b), '--', color=color)
    plt.loglog(times, palindromes, '.', color=color, label=labels[i])
    exp10 = np.round(np.log10(b))
    if n >= 3 and i == 1:
        va = 'top'
    elif n >= 3 and i == 2:
        va = 'bottom'
    else:
        va = 'center'

    plt.text(650, power_law(600, a, b), "$p=10^{%d} \\times t^{\,%.2f}$"%(exp10, a), va=va, color=color, fontsize=12)
    plt.subplots_adjust(right=0.77)

    return (times, palindromes)

def main(result_paths):
    for n in range(1, len(result_paths) + 1):
        for i, path in list(enumerate(result_paths[:n]))[::-1]:
            times, palindromes = get_file_data(i, path, n)

        plt.xlim(1, 600)
        plt.ylim(1, plt.ylim()[1])
        plt.xlabel("Time (s)", fontsize=12)
        plt.ylabel("Palindrome", fontsize=12)
        plt.title("Time to find palindromes", fontsize=12)
        legend = plt.legend(loc='lower right', ncol=2) # loc='center left',bbox_to_anchor=(1, 0)

        plt.savefig('%d.png' % n)
        plt.clf()
    # plt.show()




if __name__ == '__main__':
    main(["naive.txt", "mirror.txt", "tree_search.txt", "pruned_ranges.txt", "pruned_table.txt", "rust.txt"])
