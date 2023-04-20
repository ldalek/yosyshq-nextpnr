from os import path
import sys
sys.path.append(path.join(path.dirname(__file__), "../.."))
from himbaechel_dbgen.chip import *

# Grid size including IOBs at edges
X = 100
Y = 100
# LUT input count
K = 4
# SLICEs per tile
N = 8
# number of local wires
Wl = N * (K + 1) + 16
# 1/Fc for bel input wire pips; local wire pips and neighbour pips
Si = 6
Sq = 6
Sl = 1

dirs = [ # name, dx, dy
    ("N", 0, -1),
    ("NE", 1, -1),
    ("E", 1, 0),
    ("SE", 1, 1),
    ("S", 0, 1),
    ("SW", -1, 1),
    ("W", -1, 0),
    ("NW", -1, -1)
]

def create_switch_matrix(tt: TileType, inputs: list[str], outputs: list[str]):
    # FIXME: terrible routing matrix, just for a toy example...
    # switch wires
    for i in range(Wl):
        tt.create_wire(f"SWITCH{i}", "SWITCH")
    # neighbor wires
    for i in range(Wl):
        for d, dx, dy in dirs:
            tt.create_wire(f"{d}{i}", f"NEIGH_{d}")
    # input pips
    for i, w in enumerate(inputs):
        for j in range((i % Si), Wl, Si):
            tt.create_pip(f"SWITCH{j}", w)
    # output pips
    for i, w in enumerate(outputs):
        for j in range((i % Sq), Wl, Sq):
            tt.create_pip(w, f"SWITCH{j}")
    # neighbour local pips
    for i in range(Wl):
        for j, (d, dx, dy) in enumerate(dirs):
            tt.create_pip(f"{d}{(i + j) % Wl}", f"SWITCH{i}")
    # clock "ladder"
    if not tt.has_wire("CLK"):
        tt.create_wire(f"CLK", "TILE_CLK")
    tt.create_wire(f"CLK_PREV", "CLK_ROUTE")
    tt.create_pip(f"CLK_PREV", f"CLK")

def create_logic_tiletype(chip: Chip):
    tt = chip.create_tile_type("LOGIC")
    # setup wires
    inputs = []
    outputs = []
    for i in range(N):
        for j in range(K):
            inputs.append(f"L{i}_I{j}")
            tt.create_wire(f"L{i}_I{j}", "LUT_INPUT")
        tt.create_wire(f"L{i}_D", "FF_DATA")
        tt.create_wire(f"L{i}_O", "LUT_OUT")
        tt.create_wire(f"L{i}_Q", "FF_OUT")
        outputs += [f"L{i}_O", f"L{i}_Q"]
    tt.create_wire(f"CLK", "TILE_CLK")
    # create logic cells
    for i in range(N):
        # LUT
        lut = tt.create_bel(f"L{i}_LUT", "LUT", z=(i*2 + 0))
        for j in range(K):
            tt.add_bel_pin(lut, f"I[{j}]", f"L{i}_I{j}", PinType.INPUT)
        tt.add_bel_pin(lut, "O", f"L{i}_O", PinType.OUTPUT)
        # FF data can come from LUT output or LUT I3
        tt.create_pip(f"L{i}_O", f"L{i}_D")
        tt.create_pip(f"L{i}_I{K-1}", f"L{i}_D")
        # FF
        ff = tt.create_bel(f"L{i}_FF", "FF", z=(i*2 + 1))
        tt.add_bel_pin(ff, "D", f"L{i}_D", PinType.INPUT)
        tt.add_bel_pin(ff, "CLK", "CLK", PinType.INPUT)
        tt.add_bel_pin(ff, "Q", f"L{i}_Q", PinType.OUTPUT)
    create_switch_matrix(tt, inputs, outputs)
    return tt

N_io = 2

def create_io_tiletype(chip: Chip):
    tt = chip.create_tile_type("IO")
     # setup wires
    inputs = []
    outputs = []
    for i in range(N_io):
        tt.create_wire(f"IO{i}_T", "IO_T")
        tt.create_wire(f"IO{i}_I", "IO_I")
        tt.create_wire(f"IO{i}_O", "IO_O")
        tt.create_wire(f"IO{i}_PAD", "IO_PAD")
        inputs += [f"IO{i}_T", f"IO{i}_I"]
        outputs += [f"IO{i}_O", ]
    tt.create_wire(f"CLK", "TILE_CLK")
    for i in range(N_io):
        io = tt.create_bel(f"IO{i}", "IO", z=i)
        tt.add_bel_pin(io, "I", f"IO{i}_I", PinType.INPUT)
        tt.add_bel_pin(io, "T", f"IO{i}_T", PinType.INPUT)
        tt.add_bel_pin(io, "O", f"IO{i}_O", PinType.OUTPUT)
        tt.add_bel_pin(io, "PAD", f"IO{i}_PAD", PinType.INOUT)
    create_switch_matrix(tt, inputs, outputs)
    return tt

def create_bram_tiletype(chip: Chip):
    Aw = 9
    Dw = 16

    tt = chip.create_tile_type("BRAM")
    inputs = [f"RAM_WA{i}" for i in range(Aw)]
    inputs += [f"RAM_RA{i}" for i in range(Aw)]
    inputs += [f"RAM_WE{i}" for i in range(Dw // 8)]
    inputs += [f"RAM_DI{i}" for i in range(Dw)]
    outputs = [f"RAM_DO{i}" for i in range(Dw)]
    for w in inputs:
        tt.create_wire(w, "RAM_IN")
    for w in outputs:
        tt.create_wire(w, "RAM_OUT")
    tt.create_wire(f"CLK", "TILE_CLK")
    ram = tt.create_bel(f"RAM", F"BRAM_{2**Aw}X{Dw}", z=0)
    tt.add_bel_pin(ram, "CLK", f"CLK", PinType.INPUT)
    for i in range(Aw):
        tt.add_bel_pin(ram, f"WA[{i}]", f"RAM_WA{i}", PinType.INPUT)
        tt.add_bel_pin(ram, f"RA[{i}]", f"RAM_RA{i}", PinType.INPUT)
    for i in range(Dw//8):
        tt.add_bel_pin(ram, f"WE[{i}]", f"RAM_WE{i}", PinType.INPUT)
    for i in range(Dw):
        tt.add_bel_pin(ram, f"DI[{i}]", f"RAM_DI{i}", PinType.INPUT)
        tt.add_bel_pin(ram, f"DO[{i}]", f"RAM_DO{i}", PinType.OUTPUT)
    create_switch_matrix(tt, inputs, outputs)
    return tt

def main():
    ch = Chip("example", "EX1", X, Y)
    logic = create_logic_tiletype(ch)
    io = create_io_tiletype(ch)
    bram = create_bram_tiletype(ch)

if __name__ == '__main__':
    main()
