import numpy as np
import matplotlib.pyplot as plt


def lognormal(x, m, s):
    return np.exp(- (np.log(x) - m)**2 / (2 * s**2)) / (x * s * np.sqrt(2* np.pi))
data = np.genfromtxt("sample")
mean, std, data = data[0], data[1], data[2:]

plt.hist(data, bins = 100, density=True, alpha=0.5)
plt.gca().set_yscale("log")

x = np.linspace(np.maximum(np.min(data), 1), np.max(data), 100)
print(np.min(data), np.max(data))
y = lognormal(x, mean, std)
print(y)

plt.plot(x, y)

plt.savefig("sample.png")
