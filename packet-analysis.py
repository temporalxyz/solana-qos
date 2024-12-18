from collections import defaultdict, deque
from glob import glob
import matplotlib.pyplot as plt
import numpy as np

keys = [
    "1.1.1.1",
    "2.2.2.2",
    "7.7.7.7",
]
bins = np.logspace(1, 9, 51)


plt.figure(figsize=(8,12))

plt.subplot(211)
inputs = np.load("input.npz")
print("Input Packets")
for ip in keys:
    print(ip, len(inputs[ip]))
    plt.hist(inputs[ip], bins=bins, alpha=0.5, density=False, label=ip)
plt.gca().set_xscale("log")
plt.legend()
plt.title("TPU/FWD -> QoS Input Packets", fontsize = 15)
plt.ylabel("Number of transactions", fontsize = 15)

print("\nPost Filter + Dedup + Qos Packets")
plt.subplot(212)
outputs = np.load("output.npz")
for ip in keys:
    print(ip, len(outputs[ip]))

    plt.hist(outputs[ip], bins=bins, alpha=0.5, density=False, label=ip)
    
plt.gca().set_xscale("log")
plt.legend()
plt.title("QoS -> Banking Output Packets", fontsize = 15)
plt.xlabel("Transaction Value (Lamports per MCU)", fontsize = 15)
plt.ylabel("Number of transactions", fontsize = 15)

plt.savefig("ips")

