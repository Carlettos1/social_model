import matplotlib.pyplot as plt
import matplotlib.patches as patches
import matplotlib.animation as animation
import json
import re
import os
import matplotlib
import subprocess as sub

matplotlib.use("TkAgg")

# File paths
tmp = open("log_index", "r")
ID = int(tmp.readline())
tmp.close()
file1_path = f"log/log_{ID}.log"
file2_path = f"system/system_{ID}.log"

# Color map for opinions
COLORS = ["red", "green"]

# Set up the figure and subplots using the AAAE layout
fig, axs = plt.subplot_mosaic("AAAA\nAAAA\nAAAA\nEEEE", figsize=(10, 6))
AX_A = axs["A"]
AX_E = axs["E"]

# Initialize plots
scatter_trust = AX_A.scatter([], [], c=[], marker=".")
scatter_liar = AX_A.scatter([], [], c=[], marker="x")
(energy_line,) = AX_E.plot([], [], color="black")
AX_E.set_ylim(-8000, -4000)
AX_A.set_xlim(0, 1)
AX_A.set_ylim(0, 1)

energy_data = []


def get_last_line(path):
    with open(path, "rb") as f:
        f.seek(-2, os.SEEK_END)
        while f.read(1) != b"\n":
            f.seek(-2, os.SEEK_CUR)
        return f.readline().decode()


def update(frame):
    # Clear izones
    for patch in AX_A.patches:
        patch.remove()

    # --- Read and parse log line (file1) ---
    log_line = get_last_line(file1_path)
    match = re.search(r"Energy:\s*(-?\d+\.?\d*)", log_line)
    energy = float(match.group(1)) if match else 0
    energy_data.append(energy)

    # Adjust y-limits if energy goes outside bounds
    if energy < AX_E.get_ylim()[0]:
        AX_E.set_ylim(energy - 500, AX_E.get_ylim()[1])
    if energy > AX_E.get_ylim()[1]:
        AX_E.set_ylim(AX_E.get_ylim()[0], energy + 500)

    energy_line.set_data(range(len(energy_data)), energy_data)
    AX_E.set_xlim(0, len(energy_data))

    # --- Read and parse system state (file2) ---
    state_line = get_last_line(file2_path)
    try:
        state = json.loads(state_line)
    except json.JSONDecodeError:
        return

    ppl = state["ppl"]
    izones = state["izones"]

    X_trust, Y_trust, C_trust = [], [], []
    X_liar, Y_liar, C_liar = [], [], []

    for person in ppl:
        x, y = person["pos"]
        po, io = person["po"], person["io"]
        color = COLORS[po]
        if po == io:
            X_trust.append(x)
            Y_trust.append(y)
            C_trust.append(color)
        else:
            X_liar.append(x)
            Y_liar.append(y)
            C_liar.append(color)

    for zone in izones:
        shape = zone["shape"]
        color = COLORS[zone["opinion"]]
        alpha = 0.05 + 0.1 * abs(zone["strength"])
        if "Square" in shape:
            bl = shape["Square"]["bottom_left"]
            tr = shape["Square"]["top_right"]
            w, h = tr[0] - bl[0], tr[1] - bl[1]
            rect = patches.Rectangle(bl, w, h, alpha=alpha, facecolor=color)
            AX_A.add_patch(rect)

    scatter_trust.set_offsets(list(zip(X_trust, Y_trust)))
    scatter_trust.set_color(C_trust)
    scatter_liar.set_offsets(list(zip(X_liar, Y_liar)))
    scatter_liar.set_color(C_liar)

    sub.run(["wc", "-l", f"log/log_{ID}.log"])

    return scatter_trust, scatter_liar, energy_line


ani = animation.FuncAnimation(fig, update, interval=100, blit=False)
plt.tight_layout()
plt.show()
