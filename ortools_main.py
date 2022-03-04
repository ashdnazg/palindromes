from ortools.sat.python import cp_model

class PalindromeSolutionPrinter(cp_model.CpSolverSolutionCallback):
    """Print intermediate solutions."""

    def __init__(self, bin_vars, bin_params, dec_vars, dec_params):
        cp_model.CpSolverSolutionCallback.__init__(self)
        self.bin_vars = bin_vars
        self.bin_params = bin_params
        self.dec_vars = dec_vars
        self.dec_params = dec_params
        self.solution_count = 0

    def on_solution_callback(self):
        self.solution_count += 1
        print(sum([self.Value(bv) * bp for bv, bp in zip(self.bin_vars, self.bin_params)]))
        print(sum([self.Value(dv) * dp for dv, dp in zip(self.dec_vars, self.dec_params)]))
        print()

    def get_solution_count(self):
        return self.solution_count

def find_palindromes(bin_length, dec_length):
    bin_params = []
    for i in range((bin_length + 1) >> 1):
        bin_num = "".join(str(int(i == j or (bin_length - i - 1 == j))) for j in range(bin_length))
        bin_params.append(int(bin_num, 2))
    print(bin_params)

    dec_params = []
    for i in range((dec_length + 1) >> 1):
        dec_num = "".join(str(int(i == j or (dec_length - i - 1 == j))) for j in range(dec_length))
        dec_params.append(int(dec_num, 10))
    print(dec_params)

    model = cp_model.CpModel()

    # Creates the variables
    bin_vars = [model.NewIntVar(1, 1, f'b0')]
    print(dir(bin_vars[0]))
    exit(0)
    bin_vars += [model.NewIntVar(0, 1, f'b{i}') for i in range(1, len(bin_params))]
    dec_vars = [model.NewIntVar(1, 9, f'd0')]
    dec_vars += [model.NewIntVar(0, 9, f'd{i}') for i in range(1, len(dec_params))]

    # Create the constraints.
    model.Add(sum([bv * bp for bv, bp in zip(bin_vars, bin_params)]) == sum([dv * dp for dv, dp in zip(dec_vars, dec_params)]))

    # Create a solver and solve.
    solver = cp_model.CpSolver()
    # solver.parameters.log_search_progress = True
    solution_printer = PalindromeSolutionPrinter(bin_vars, bin_params, dec_vars, dec_params)
    status = solver.SearchForAllSolutions(model, solution_printer)

    print('Status = %s' % solver.StatusName(status))
    print('Number of solutions found: %i' % solution_printer.get_solution_count())

num = 5652622262565

find_palindromes(len(bin(num)) - 2, len(str(num)))
