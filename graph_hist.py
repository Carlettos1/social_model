import sys
import numpy as np
import matplotlib.pyplot as plt


ID = 0
if len(sys.argv) < 2:
    ID = "last"
else:
    ID = sys.argv[1]

if ID == "last":
    tmp = open("log_index", "r")
    ID = int(tmp.readline())
    tmp.close()

file = f"histeresis/g_vs_m_{ID}.log"
data = np.loadtxt(file, delimiter=",")
g = data[4_000:,0]
m = data[4_000:,1]

plt.plot(g, m, "bo", markersize=3)
plt.xlabel("Campo Magnético")
plt.ylabel("Magnetización")
plt.axhline(0, color="black", linewidth=0.5)
plt.axvline(0, color="black", linewidth=0.5)
plt.show()
