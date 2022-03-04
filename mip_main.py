import mip

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
    model = mip.Model(solver_name="cbc")

    bin_vars = [model.add_var(f'b0', 1, 1, var_type=mip.BINARY)]
    bin_vars += [model.add_var(f'b{i}', 0, 1, var_type=mip.BINARY) for i in range(1, len(bin_params))]
    dec_vars = [model.add_var(f'd0', 1, 9, var_type=mip.INTEGER)]
    dec_vars += [model.add_var(f'd{i}', 0, 9, var_type=mip.INTEGER) for i in range(1, len(dec_params))]
    model.add_constr(sum([bv * bp for bv, bp in zip(bin_vars, bin_params)]) == sum([dv * dp for dv, dp in zip(dec_vars, dec_params)]))

    status = model.optimize()
    print(status)
    print(sum([bv.x * bp for bv, bp in zip(bin_vars, bin_params)]))
    print(sum([dv.x * dp for dv, dp in zip(dec_vars, dec_params)]))

# mm = mip.Model(solver_name="cbc")
# mm.verbose = False

# # Blocks
# bus = mm.add_var("bus", 0, 1, var_type=mip.BINARY)
# battery_1 = mm.add_var("battery_1", 0, 1, var_type=mip.BINARY)
# battery_2 = mm.add_var("battery_2", 0, 1, var_type=mip.BINARY)

# # Ports
# bus_p1 = mm.add_var("bus_p1", 0, 1, var_type=mip.BINARY)
# bus_p2 = mm.add_var("bus_p2", 0, 1, var_type=mip.BINARY)
# battery_1_p = mm.add_var("battery_1_p", 0, 1, var_type=mip.BINARY)
# battery_2_p = mm.add_var("battery_2_p", 0, 1, var_type=mip.BINARY)

# # Constraints
# mm.add_constr(battery_1 + battery_2 >= 0)
# mm.add_constr(battery_1 + battery_2 <= 2)
# mm.add_constr(bus == 1)
# mm.add_constr(battery_1 >= battery_2)
# mm.add_constr(battery_1 == battery_1_p)
# mm.add_constr(battery_2 == battery_2_p)
# mm.add_constr(bus >= bus_p1)
# mm.add_constr(bus >= bus_p2)
# mm.add_constr(bus_p1 <= bus_p2)
# mm.add_constr(bus_p1 + bus_p2 == battery_1_p + battery_2_p)

# count = 0
# for i in range(0, 10):
#     status = mm.optimize()
#     if status == status.FEASIBLE or status == status.OPTIMAL:
#         print(f"{bus}: {bus.x}")
#         print(f"{battery_1}: {battery_1.x}")
#         print(f"{battery_2}: {battery_2.x}")
#     else:
#         print("INFEASIBLE")
#         break
#     count += 1
#     # or(a != a.x, b != b.x, c != by)
#     flip = [1 - v if v.x else v for v in (bus, battery_1, battery_2)]
#     mm.add_constr(mip.xsum(flip) >= 1)
# print(f"CBC (Python-MIP) found {count} solutions")



num = 7227526257227

find_palindromes(len(bin(num)) - 2, len(str(num)))