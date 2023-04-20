from dataclasses import dataclass, field
from .bba import BBAWriter
from enum import Enum
import abc
import struct, hashlib

"""
This provides a semi-flattened routing graph that is built into a deduplicated one.

There are two key elements:
 - Tile Types:
      these represent a unique kind of grid location in terms of its contents:
       - bels (logic functionality like LUTs, FFs, IOs, IP, etc)
       - internal wires (excluding connectivity to other tiles)
       - pips that switch internal wires
 - Nodes
      these merge tile-internal wires across wires to create inter-tile connectivity
      so, for example, a length-4 wire might connect (x, y, "E4AI") and (x+3, y, "E4AO")
"""

@dataclass(eq=True, frozen=True)
class IdString:
    index: int = 0

class StringPool:
    def __init__(self):
        self.strs = {"": 0}

    def id(self, val: str):
        if val in self.strs:
            return IdString(self.strs[val])
        else:
            idx = len(self.strs)
            self.strs[val] = idx
            return IdString(idx)

@dataclass
class PinType(Enum):
    INPUT = 0
    OUTPUT = 1
    INOUT = 2

class BBAStruct(abc.ABC):
    def serialise_lists(self, bba: BBAWriter):
        pass
    def serialise(self, bba: BBAWriter):
        pass

@dataclass
class BelPin:
    name: IdString
    wire: int
    dir: PinType

BEL_FLAG_GLOBAL = 0x01
BEL_FLAG_HIDDEN = 0x02

@dataclass
class BelData(BBAStruct):
    index: int
    name: IdString
    bel_type: IdString
    z: int

    flags: int = 0
    site: int = 0
    checker_idx: int = 0

    pins: list[BelPin] = field(default_factory=list)
    extra_data: object = None

@dataclass
class BelPinRef:
    bel: int
    pin: IdString

@dataclass
class TileWireData:
    index: int
    name: IdString
    wire_type: IdString
    flags: int = 0

    # not serialised, but used to build the global constant networks
    const_val: int = -1

    # these crossreferences will be updated by finalise(), no need to manually update
    pips_uphill: list[int] = field(default_factory=list)
    pips_downhill: list[int] = field(default_factory=list)
    bel_pins: list[BelPinRef] = field(default_factory=list)

@dataclass
class PipData:
    index: int
    src_wire: int
    dst_wire: int
    pip_type: IdString = field(default_factory=IdString)
    flags: int = 0
    timing_idx: int = -1

@dataclass
class TileType:
    strs: StringPool
    type_name: IdString
    bels: list[BelData] = field(default_factory=list)
    pips: list[PipData] = field(default_factory=list)
    wires: list[TileWireData] = field(default_factory=list)

    _wire2idx: dict[IdString, int] = field(default_factory=dict)

    def create_bel(self, name: str, type: str, z: int=-1):
        bel = BelData(index=len(self.bels),
            name=self.strs.id(name),
            bel_type=self.strs.id(type),
            z=z)
        self.bels.append(bel)
        return bel
    def add_bel_pin(self, bel: BelData, pin: str, wire: str, dir: PinType):
        pin_id = self.strs.id(pin)
        wire_idx = self._wire2idx[self.strs.id(wire)]
        bel.pins.append(BelPin(pin_id, wire_idx, dir))
        self.wires[wire_idx].bel_pins.append(BelPinRef(bel.index, pin_id))

    def create_wire(self, name: str, type: str=""):
        wire = TileWireData(index=len(self.wires),
            name=self.strs.id(name),
            wire_type=self.strs.id(type))
        self._wire2idx[wire.name] = wire.index
        self.wires.append(wire)
        return wire
    def create_pip(self, src: str, dst: str):
        src_idx = self._wire2idx[self.strs.id(src)]
        dst_idx = self._wire2idx[self.strs.id(dst)]
        pip = PipData(index=len(self.pips), src_wire=src_idx, dst_wire=dst_idx)
        return pip
    def has_wire(self, wire: str):
        return self.strs.id(wire) in self._wire2idx


# Pre deduplication (nodes flattened, absolute coords)
@dataclass
class NodeWire:
    x: int
    y: int
    wire: IdString

# Post deduplication (node shapes merged, relative coords)
@dataclass
class TileWireRef:
    dx: int
    dy: int
    wire: int

@dataclass
class NodeShape:
    wires: list[TileWireRef] = field(default_factory=list)
    def key(self):
        m = hashlib.sha1()
        for wire in self.wires:
            m.update(wire.dx.to_bytes(2, 'little'))
            m.update(wire.dy.to_bytes(2, 'little'))
            m.update(wire.wire.to_bytes(2, 'little'))
        return m.digest()

MODE_TILE_WIRE = 0x7000
MODE_IS_ROOT = 0x7001
MODE_ROW_CONST = 0x7002
MODE_GLB_CONST = 0x7003

@dataclass
class RelNodeRef:
    dx_mode: int = MODE_TILE_WIRE
    dy: int = 0
    wire: int = 0

@dataclass
class TileRoutingShape:
    wire_to_node: list[RelNodeRef]
    def key(self):
        m = hashlib.sha1()
        for wire in self.wire_to_node:
            m.update(wire.dx_mode.to_bytes(2, 'little'))
            m.update(wire.dy.to_bytes(2, 'little'))
            m.update(wire.wire.to_bytes(2, 'little'))
        return m.digest()

@dataclass
class TileInst:
    x: int
    y: int
    type_idx: int = -1
    name_prefix: str = ""
    loc_type: int = 0
    wire_to_node: list[RelNodeRef] = field(default_factory=list)

class Chip:
    def __init__(self, uarch: str, name: str, width: int, height: int):
        self.strs = StringPool()
        self.uarch = uarch
        self.name = name
        self.width = width
        self.height = height
        self.tile_types = []
        self.tiles = [[TileInst(x, y) for x in range(width)] for y in range(height)]
        self.tile_type_idx = dict()
        self.node_shapes = []
        self.node_shape_idx = dict()
    def create_tile_type(self, name: str):
        tt = TileType(self.strs, self.strs.id(name))
        self.tile_type_idx[name] = len(self.tile_types)
        self.tile_types.append(tt)
        return tt
    def set_tile_type(self, x: int, y: int, type: str):
        self.tiles[y][x].type_idx = self.tile_type_idx[type]
    def tile_type_at(self, x: int, y: int):
        return self.tile_types[self.tiles[y][x].type_idx]
    def add_node(self, wires: list[NodeWire]):
        # both the tile types and tile inst->type map must be set up before calling this function
        x0 = wires[0].x
        y0 = wires[0].y
        # compute node shape
        shape = NodeShape()
        for w in wires:
            shape.wires.append(TileWireRef(
                dx=w.x-x0, dy=w.y-y0,
                wire=self.tile_type_at(w.x, w.y)._wire2idx[w.wire]
            ))
        # deduplicate node shapes
        key = shape.key()
        if key in self.node_shape_idx:
            shape_idx = self.node_shape_idx[key]
        else:
            shape_idx = len(self.node_shapes)
            self.node_shape_idx[key] = shape_idx
            self.node_shapes.append(shape)
        # update tile wire to node ref
        for i, w in enumerate(wires):
            inst = self.tiles[w.y][w.x]
            wire_idx = shape.wires[i].wire
            # make sure there's actually enough space; first
            if wire_idx >= len(inst.wire_to_node):
                inst.wire_to_node += [RelNodeRef() for k in range(len(inst.wire_to_node), wire_idx+1)]
            if i == 0:
                # root of the node. we don't need to back-reference anything because the node is based here
                # so we re-use the structure to store the index of the node shape, instead
                inst.wire_to_node[wire_idx] = RelNodeRef(MODE_IS_ROOT, (shape_idx & 0xFFFF), ((shape_idx >> 16) & 0xFFFF))
            else:
                # back-reference to the root of the node
                dx = w.x - x0
                dy = w.y - y0
                assert dx < MODE_TILE_WIRE, "dx range causes overlap with magic values!"
                inst.wire_to_node[wire_idx] = RelNodeRef(dx, dy, shape.wires[0].wire)

