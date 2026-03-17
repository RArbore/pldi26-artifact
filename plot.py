from matplotlib import pyplot as plt
import numpy as np

data = np.loadtxt("plot_data.csv", delimiter=" ", dtype=float)
min_outer_iters = int(min(data[:, 2]))
max_outer_iters = int(max(data[:, 2]))

fig, ax = plt.subplots()
ax.tick_params(axis='both', which='major', labelsize=15)

colors = {
    2: "blue",
    3: "green",
    4: "orange",
    5: "red",
    6: "purple",
    7: "black",
}

max_x = np.max(data[:, 0])
for outer_iters in range(min_outer_iters, max_outer_iters + 1):
    points = data[data[:, 2] == outer_iters, :2]
    points[:, 1] /= 1000.0
    plt.scatter(points[:, 0], points[:, 1], label=f"$n = {outer_iters}$", color=colors[outer_iters], s=10.0)

plt.xlabel("Number of e-nodes", fontsize=15)
plt.ylabel("Wall clock time (ms)", fontsize=15)
plt.legend(fontsize=15)
plt.show()
