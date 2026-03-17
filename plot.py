from matplotlib import pyplot as plt
import numpy as np

data = np.loadtxt("plot_data.csv", delimiter=" ", dtype=int)
min_outer_iters = min(data[:, 2])
max_outer_iters = max(data[:, 2])

fig, ax = plt.subplots()
ax.tick_params(axis='both', which='major', labelsize=20)

colors = {
    2: "blue",
    3: "green",
    4: "orange",
    5: "red",
}

max_x = np.max(data[:, 0])
for outer_iters in range(min_outer_iters, max_outer_iters + 1):
    points = np.asarray(data[data[:, 2] == outer_iters, :2], dtype=float)
    points[:, 1] /= 1000.0
    plt.scatter(points[:, 0], points[:, 1], label=f"{outer_iters} outer iterations", color=colors[outer_iters])

plt.xlabel("# e-nodes", fontsize=20)
plt.ylabel("Wall clock time (ms)", fontsize=20)
plt.legend(fontsize=20)
plt.show()
