import sys
import math
import re
import threading
import numpy as np
import matplotlib.scale as scale
import matplotlib.transforms as tr
import matplotlib.pyplot as plt
import matplotlib.patches as patches
import pandas as pd
import subprocess as sub
from sshkeyboard import listen_keyboard
import struct

import matplotlib
matplotlib.use("TkAgg")

FRAMES = 20_000 - 1
DELAY = 10

## %% FOR BINARY SYSTEM DESERIALIZATION
## Explained in readme.md for f16u8
SYSTEM_EXPECTED_BYTES = 812

ID = 0
if len(sys.argv) < 2:
    ID = "last"
else:
    ID = sys.argv[1]

if ID == "last":
    tmp = open("log_index", "r")
    ID = int(tmp.readline())
    tmp.close()

B0_FILE = open(f"b0/b0_{ID}.log", "r")
B1_FILE = open(f"b1/b1_{ID}.log", "r")
SYSTEM_FILE = open(f"system/system_{ID}.log", "rb")
LOG_FILE = open(f"log/log_{ID}.log", "r")

## Realmente ubyte es suficiente para n<255, pero uso ushort para n~10k en testing
B0_LOG = np.fromfile(B0_FILE, dtype="ushort", sep=", ")
B1_LOG = np.fromfile(B1_FILE, dtype="ushort", sep=", ")
LOG = LOG_FILE.read()
FRAMES = B0_LOG.shape[0]

#B0_ITER = iter(B0_LOG)
#B1_ITER = iter(B1_LOG)
#LOG_ITER = iter(LOG.splitlines())
#SYSTEM_ITER = SYSTEM_FILE

fig, ax = plt.subplot_mosaic("AAB\nAAC\nAAD", figsize=(10,6))
SYSTEM_PLOT: plt.Axes = ax["A"]
ENERGY_PLOT: plt.Axes = ax["B"]
B_PLOT: plt.Axes = ax["C"]
REJ_PLOT: plt.Axes = ax["D"]
PATCHES: list = []

ENERGY_ALLDATA = pd.DataFrame([ float(re.search(r"Energy: +(-?\d+(?:\.\d+)?)", log).group(1)) for log in LOG.splitlines() ])
B0_ALLDATA = pd.DataFrame(B0_LOG)
B1_ALLDATA = pd.DataFrame(B1_LOG)
REJ_ALLDATA = pd.DataFrame([ float(re.search(r"rej\/n: (\d\.\d+)", log).group(1)) for log in LOG.splitlines() ])

SMA = 100
ENERGY_SMA = ENERGY_ALLDATA.rolling(SMA).mean()
B0_SMA = B0_ALLDATA.rolling(SMA).mean()
B1_SMA = B1_ALLDATA.rolling(SMA).mean()

class Currents:
    def __init__(self) -> None:
        self.sys = None
        self.flag = True
        self.index = 0
        self.energy = 0
        self.energy_sma = 0
        self.b0 = 0
        self.b0_sma = 0
        self.b1 = 0
        self.b1_sma = 0
        self.rej = 0
        pass

    def release(self):
        self.flag = True

    def press(self):
        self.flag = False

    def next_line(self):
        self.sys = SYSTEM_FILE.read(SYSTEM_EXPECTED_BYTES)
        assert len(self.sys) == SYSTEM_EXPECTED_BYTES, f"{len(self.sys)} out of {SYSTEM_EXPECTED_BYTES}" # expect to be exactly this ammount of bytes
        assert next(SYSTEM_FILE) == b'\n' # each system is separated by a \n
        self.energy = ENERGY_ALLDATA[0][self.index]
        self.energy_sma = np.nan_to_num(ENERGY_SMA[0][self.index], nan=self.energy)
        self.b0 = B0_ALLDATA[0][self.index]
        self.b0_sma = np.nan_to_num(B0_SMA[0][self.index], nan=self.b0)
        self.b1 = B1_ALLDATA[0][self.index]
        self.b1_sma = np.nan_to_num(B1_SMA[0][self.index], nan=self.b1)
        self.rej = REJ_ALLDATA[0][self.index]
        self.index += 1

CURRENTS = Currents()
COLORS = ["r", "g", "b", "black"]

def read_current_system():
    # len: u8
    len_ppl_vec = struct.unpack("<B", CURRENTS.sys[:1])[0]
    B = [0] * len_ppl_vec
    S = [0] * len_ppl_vec
    X = [0] * len_ppl_vec
    Y = [0] * len_ppl_vec
    # ppl: io-x + po-y = f16 + f16 (signo = io/po)
    for i in range(len_ppl_vec):
        a = 1 + 4 * i
        b = a + 2
        c = b + 2
        io_x = struct.unpack("<e", CURRENTS.sys[a:b])[0]
        po_y = struct.unpack("<e", CURRENTS.sys[b:c])[0]
        ppl_x = abs(io_x)
        ppl_y = abs(po_y)
        ppl_io = 0 if io_x >= 0 else 1
        ppl_po = 0 if po_y >= 0 else 1
        # assert ppl_io == 1 or ppl_io == 0, f"({i}) {ppl_io = }"
        # assert ppl_po == 1 or ppl_po == 0, f"({i}) {ppl_po = }"
        # assert 0 <= ppl_x <= 1, f"({i}) {ppl_x = }"
        # assert 0 <= ppl_y <= 1, f"({i}) {ppl_y = }"
        B[i] = ppl_io
        S[i] = ppl_po
        X[i] = ppl_x
        Y[i] = ppl_y
    last_byte_read = c
    # next should be len_iz_vec == 1
    len_iz_vec = struct.unpack('<B', CURRENTS.sys[last_byte_read:last_byte_read + 1])[0]
    last_byte_read += 1
    izones = CURRENTS.sys[last_byte_read:]
    sqr = []
    # izone: bl + tr + str-op = f16*2 + f16*2 + f16 (signo = op)
    for i in range(len_iz_vec):
        # for square: bottom_left(xy), top_right(xy), opinion, strength
        a = i * 10
        b = a + 2
        c = b + 2
        d = c + 2
        e = d + 2
        f = e + 2
        iz_blx = struct.unpack("<e", izones[a:b])[0]
        iz_bly = struct.unpack("<e", izones[b:c])[0]
        iz_trx = struct.unpack("<e", izones[c:d])[0]
        iz_try = struct.unpack("<e", izones[d:e])[0]
        str_op = struct.unpack("<e", izones[e:f])[0]
        iz_str = abs(str_op)
        iz_op = 0 if str_op >= 0 else 1
        # assert 0 <= iz_blx <= 1, f"({i}) {iz_blx = }"
        # assert 0 <= iz_bly <= 1, f"({i}) {iz_bly = }"
        # assert 0 <= iz_trx <= 1, f"({i}) {iz_trx = }"
        # assert 0 <= iz_try <= 1, f"({i}) {iz_try = }"
        # assert iz_op == 0 or iz_op == 1, f"({i}) {iz_op = }"
        sqr.append([iz_blx, iz_bly, iz_trx, iz_try, iz_op, iz_str])
    return B, S, X, Y, sqr

def update_system(frame):
    B, S, X, Y, sqr = read_current_system()

    POST, ST, POSL, SL = separate(B, S, X, Y)

    SYSTEM_LSCATTER.set_offsets(POSL)
    SYSTEM_LSCATTER.set_color(SL)
    SYSTEM_TSCATTER.set_offsets(POST)
    SYSTEM_TSCATTER.set_color(ST)

    for i, r in enumerate(sqr):
        alpha = 0.5 * r[5] + 0.1
        alpha = 0.9 if alpha > 0.9 else alpha
        facecolor = COLORS[int(r[4])]
        PATCHES[i].set_facecolor(facecolor)
        PATCHES[i].set_alpha(alpha)
    pass

def separate(B, S, X, Y):
    liars = [s != b for (s, b) in zip(S, B)]
    SL, ST = [], []
    POSL, POST = [], []

    SS = [COLORS[s] for s in S]
    # trustful, liars
    for i in range(len(S)):
        if liars[i]:
            POSL.append((X[i], Y[i]))
            SL.append(SS[i])
        else:
            POST.append((X[i], Y[i]))
            ST.append(SS[i])
    return POST, ST, POSL, SL

def update_rej(frame):
    REJ_LN.set_xdata([frame])
    REJ_LN.set_ydata([CURRENTS.rej])
    REJ_TEXT.set_x(frame)
    REJ_TEXT.set_y(CURRENTS.rej)
    REJ_TEXT.set_text(f"{CURRENTS.rej}")

    REJ_PLOT.draw_artist(REJ_LN)
    REJ_PLOT.draw_artist(REJ_TEXT)
    pass

def update_b(frame):
    B0_LN.set_xdata([frame])
    B0_LN.set_ydata([CURRENTS.b0_sma])
    B0_TEXT.set_x(frame)
    B0_TEXT.set_y(CURRENTS.b0_sma)
    B0_TEXT.set_text(f"{CURRENTS.b0_sma:.1f} ({CURRENTS.b0})")

    B1_LN.set_xdata([frame])
    B1_LN.set_ydata([CURRENTS.b1_sma])
    B1_TEXT.set_x(frame)
    B1_TEXT.set_y(CURRENTS.b1_sma)
    B1_TEXT.set_text(f"{CURRENTS.b1_sma:.1f} ({CURRENTS.b1})")

    B_PLOT.draw_artist(B0_LN)
    B_PLOT.draw_artist(B0_TEXT)
    B_PLOT.draw_artist(B1_LN)
    B_PLOT.draw_artist(B1_TEXT)
    pass

def update_energy(frame):
    ENERGY_LN.set_xdata([frame])
    ENERGY_LN.set_ydata([CURRENTS.energy_sma])
    ENERGY_TEXT.set_x(frame)
    ENERGY_TEXT.set_y(CURRENTS.energy_sma)
    ENERGY_TEXT.set_text(f"{CURRENTS.energy_sma:.1f} ({CURRENTS.energy:.1f})")

    ENERGY_PLOT.draw_artist(ENERGY_LN)
    ENERGY_PLOT.draw_artist(ENERGY_TEXT)
    pass

def init_patches():
    _,_,_,_,squares = read_current_system()
    for r in squares:
        rect = patches.Rectangle((r[0], r[1]), r[2]-r[0], r[3]-r[1], alpha=0.5*r[5]+0.1, facecolor=COLORS[int(r[4])])
        p = SYSTEM_PLOT.add_patch(rect)
        PATCHES.append(p)
    # for c in circles:
    #     cir = patches.Circle((c[0], c[1]), c[2], alpha=0.5*c[4]+0.1, facecolor=COLORS[int(c[3])])
    #     p = SYSTEM_PLOT.add_patch(cir)
    #     PATCHES.append(p)

def init():
    SYSTEM_PLOT.set_xlim(0, 1)
    SYSTEM_PLOT.set_ylim(0, 1)

    ENERGY_PLOT.set_xlim(0, FRAMES)
    ENERGY_PLOT.plot(ENERGY_ALLDATA, color="gray", alpha=0.5)
    ENERGY_PLOT.plot(ENERGY_SMA, color="gray", alpha=1.0)

    B_PLOT.set_xlim(0, FRAMES)
    B_PLOT.plot(B0_ALLDATA, color="cyan", alpha=0.3)
    B_PLOT.plot(B0_SMA, color="cyan", alpha=1.0)
    B_PLOT.plot(B1_ALLDATA, color="yellow", alpha=0.3)
    B_PLOT.plot(B1_SMA, color="yellow", alpha=1.0)

    REJ_PLOT.set_xlim(0, FRAMES)
    REJ_PLOT.set_ylim(0, 1)
    REJ_PLOT.plot(REJ_ALLDATA, color="gray", alpha=0.5)
    pass

SYSTEM_LSCATTER = SYSTEM_PLOT.scatter([0], [0], c=["r"], marker="x")
SYSTEM_TSCATTER = SYSTEM_PLOT.scatter([0], [0], c=["r"], marker=".")

ENERGY_LN, = ENERGY_PLOT.plot([0], [0], "o", animated=True)
ENERGY_TEXT = ENERGY_PLOT.text(0, 0, "")

B0_LN, = B_PLOT.plot([0], [0], "bo", animated=True)
B0_TEXT = B_PLOT.text(0, 0, f"")
B1_LN, = B_PLOT.plot([0], [0], "ro", animated=True)
B1_TEXT = B_PLOT.text(0, 0, "")

REJ_LN, = REJ_PLOT.plot([0], [0], "o", animated=True)
REJ_TEXT = REJ_PLOT.text(0, 0, "")

CURRENTS.next_line()
init()
fig.set_animated(True)
plt.show(block=False)
plt.pause(1.0)
title = fig.suptitle("Frame: 0")

bg = fig.canvas.copy_from_bbox(fig.bbox)
fig.canvas.blit()

def on_pressed(event):
    if event.key == "x":
        CURRENTS.press()

def on_release(event):
    if event.key == "x":
        CURRENTS.release()

def on_resize(event):
    global bg
    fig.canvas.draw()  # Redraw the figure so layout is correct
    bg = fig.canvas.copy_from_bbox(fig.bbox)

fig.canvas.mpl_connect("resize_event", on_resize)
fig.canvas.mpl_connect("key_press_event", on_pressed)
fig.canvas.mpl_connect("key_release_event", on_release)

init_patches()
for frame in range(FRAMES):
    fig.canvas.flush_events()
    title.set_text(f"Frame: {frame}")
    print(f"Frame {frame}")

    if CURRENTS.flag:
        fig.canvas.restore_region(bg) # <- restora el bg, quita el anterior plot
        if frame % 100 == 0:
            # fig.canvas.draw_idle()
            # fig.canvas.flush_events()
            bg = fig.canvas.copy_from_bbox(fig.bbox)
        for patch in PATCHES:
            SYSTEM_PLOT.draw_artist(patch)
        update_rej(frame)
        update_b(frame)
        update_energy(frame)
        update_system(frame)
        SYSTEM_PLOT.draw_artist(SYSTEM_LSCATTER)
        SYSTEM_PLOT.draw_artist(SYSTEM_TSCATTER)
        fig.canvas.blit(fig.bbox)
        fig.draw_artist(title)
    
    if plt.get_fignums().__len__() == 0:
        break
    CURRENTS.next_line()
