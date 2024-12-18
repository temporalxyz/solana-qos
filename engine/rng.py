import numpy as np
import matplotlib.pyplot as plt


data = np.genfromtxt("rng")

plt.hist(data, bins=100)

plt.savefig("rng.png")