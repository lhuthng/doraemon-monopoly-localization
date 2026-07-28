# Doraemon Monopoly `mapNNNN.dat` format

This document describes the map archive format as currently understood from all
eight original maps, correlations with visible game behaviour, and static
analysis of the shipped `Doraemon.exe`. All integers are little-endian unless
stated otherwise.

Executable addresses below are virtual addresses in the original 32-bit x86
image (image base `0x00400000`). They are included as short, reproducible proof,
not as names for functions whose original symbols are unavailable. The examined
executable has SHA-256
`fdf00e681671f93b09d257f77d7ce0720e7129cf6bc44ba9e0f19c2efa4fecba`.

Confidence labels used below:

- **Confirmed**: established by file boundaries, exact cross-file alignment, or
  repeatable in-game/map evidence.
- **Strong candidate**: supported by consistent correlations, but not yet traced
  to the executable routine that consumes it.
- **Unknown**: preserved exactly; no gameplay meaning is assigned.

## 1. What a “record” means

`mapNNNN.dat` is a GameOne Systems archive. A record is one child entry in that
archive, comparable to a file inside a ZIP archive. A cell is not an archive
record: the cells are fixed-size structures stored consecutively inside record
`000`.

The archive wrapper starts with:

```text
00 00 47 61 6d 65 4f 6e 65 20 53 79 73 74 65 6d
73 20 4c 69 6d 69 74 65 64 0a 57 72 69 74 74 65
6e 20 62 79 20 53 61 6d 6d 65 20 4e 47 00
```

This is `\0\0GameOne Systems Limited\nWritten by Samme NG\0`.

Important archive-wrapper fields are:

| Archive offset | Type           | Meaning                                                                           |
| -------------- | -------------- | --------------------------------------------------------------------------------- |
| `+0x42`        | `u32`          | Number of child records.                                                          |
| `+0x66`        | `u32[count+1]` | Child offsets relative to the archive start, followed by one terminal/end offset. |

Every `mapNNNN.dat` examined has four children:

| Record | Contents                                                                           | Status                                   |
| ------ | ---------------------------------------------------------------------------------- | ---------------------------------------- |
| `000`  | 64-byte map header, layer descriptors/cell grids, and BMP path catalogue           | Parsed                                   |
| `001`  | Embedded PCX map-selection preview                                                 | Parsed                                   |
| `002`  | Map-wide configuration, starts, directions, jail destination, and other raw fields | Partially parsed                         |
| `003`  | Fixed-size shop/special-location records                                           | Structurally parsed; shop role confirmed |

Record offsets below are relative to the beginning of the extracted record, not
to the beginning of `mapNNNN.dat`.

## 2. Record `000`: header, cells, and artwork paths

### 2.1 The 64-byte header and first layer descriptor

Record `000` begins with a `0x40`-byte map header. It is followed by one
`0x10`-byte descriptor for each layer, then that layer's cells. All eight
original maps have one layer, so the first cell begins at `0x50`; this is why
the header and first descriptor previously looked like one 80-byte header.

| Offset         | Type     | Interpretation                                                                                      |
| -------------- | -------- | --------------------------------------------------------------------------------------------------- |
| `+0x00`        | `u32`    | Unknown; zero in all eight maps.                                                                    |
| `+0x04`        | `u32`    | **Confirmed map width in cells.**                                                                   |
| `+0x08`        | `u32`    | **Confirmed map height in cells.**                                                                  |
| `+0x0c`        | `u32`    | Unknown packed value. It resembles a 24-bit colour in several maps, but that meaning is unverified. |
| `+0x10`        | `u32`    | Unknown packed value; same caution as `+0x0c`.                                                      |
| `+0x14..+0x2c` | `u32[7]` | Unknown; zero in all eight maps.                                                                    |
| `+0x30`        | `u32`    | **Confirmed terrain-art count.** It exactly equals the number of `mapElem` group-`000` entries.     |
| `+0x34`        | `u32`    | **Confirmed layer count.** It is one in all eight shipped maps.                                     |
| `+0x38..+0x3c` | `u32[2]` | Unknown; zero in all eight maps.                                                                    |

Each layer begins with this descriptor:

| Descriptor offset | Type     | Interpretation                                                                      |
| ----------------- | -------- | ----------------------------------------------------------------------------------- |
| `+0x00`           | `u32`    | Unknown packed value. Do not treat it as a pointer or colour without more evidence. |
| `+0x04`           | `u32`    | Unknown packed value.                                                               |
| `+0x08..+0x0c`    | `u32[2]` | Unknown; zero in all eight shipped maps.                                            |

Executable proof: at `0x477ef2` the loader copies exactly `0x40` bytes into the
base map structure. At `0x477f60` it reads `header[+0x34]` as the loop/allocation
count. Each iteration copies a `0x10`-byte descriptor at `0x478057–0x478064`,
then allocates and copies `width * height * 0x10` cell bytes at
`0x4780a4–0x478128`. Record `002` is copied separately as exactly `0x118` bytes.

The dimensions and terrain count across the originals are:

| Map | Embedded artwork name |      Size | `+0x30` terrain count |
| --- | --------------------- | --------: | --------------------: |
| `0` | Dinosaur              | `50 × 50` |                    56 |
| `1` | Ice/Snow              | `40 × 40` |                    40 |
| `2` | Tree                  | `50 × 50` |                   104 |
| `3` | Western               | `60 × 60` |                    40 |
| `4` | Doraemon Town         | `50 × 50` |                    64 |
| `5` | Future                | `50 × 50` |                    32 |
| `6` | All map               | `80 × 80` |                   528 |
| `7` | MMap02/All map        | `80 × 80` |                   528 |

### 2.2 Exact Doraemon Town header and descriptor example

The full record-`000` header in `map0004.dat` is:

```text
offset  little-endian u32
+0x00   0x00000000   unknown, always zero
+0x04   0x00000032   width  = 50
+0x08   0x00000032   height = 50
+0x0c   0x00c8f144   unknown packed value
+0x10   0x00c8f134   unknown packed value
+0x14   0x00000000   unknown
+0x18   0x00000000   unknown
+0x1c   0x00000000   unknown
+0x20   0x00000000   unknown
+0x24   0x00000000   unknown
+0x28   0x00000000   unknown
+0x2c   0x00000000   unknown
+0x30   0x00000040   64 terrain images in mapElem group 000
+0x34   0x00000001   one map layer
+0x38   0x00000000   unknown
+0x3c   0x00000000   unknown
+0x40   0x006600ff   layer 0 descriptor +0x00, unknown packed value
+0x44   0x006615b0   layer 0 descriptor +0x04, unknown packed value
+0x48   0x00000000   layer 0 descriptor +0x08, unknown
+0x4c   0x00000000   layer 0 descriptor +0x0c, unknown
```

The bytes for `width = 50`, for example, are `32 00 00 00` because the
integer is little-endian.

### 2.3 Cell array

For the shipped one-layer maps, the cell array begins at record offset `0x50`.
It contains exactly `width × height` records, each exactly `0x10` (16) bytes.

```text
cellOffset = 0x50 + ((y * width) + x) * 0x10
x          = cellIndex % width
y          = floor(cellIndex / width)
```

This proves that the file itself is a rectangular logical array. The isometric
view in Resource Studio is only a projection of this array; the bytes are not
stored in isometric screen order.

### 2.4 Every byte in one cell

```text
relative  size  type  meaning
+0x00     2     u16   layer/control word; low byte selects the effective layer
+0x02     2     u16   object/reference ID; 0xffff means no reference
+0x04     2     u16   terrain/control flags
+0x06     2     u16   terrain image ID; 0xffff means no terrain
+0x08     4     u32   price for class-1 land; otherwise preserve as raw value
+0x0c     1     u8    event class consumed by the executable
+0x0d     3     bytes unknown/padding; zero in the shipped maps
```

The same bytes viewed as four little-endian words are:

```text
word 0 = (objectId  << 16) | layerControl
word 1 = (terrainId << 16) | terrainFlags
word 2 = price/raw value
word 3 = event class in its low byte; upper 24 bits remain raw
```

Example:

```text
raw bytes:
00 00 ff ff 80 00 00 00 00 00 00 00 00 00 00 00

word +0x00 = 0xffff0000 -> object ID 0xffff (none), layer/control 0x0000
word +0x04 = 0x00000080 -> terrain ID 0, terrain flags 0x0080
word +0x08 = 0x00000000 -> raw value 0
word +0x0c = 0x00000000 -> event class 0 (none)
```

There is no hidden fifth word per cell. Every unknown per-cell value must be in
these 16 bytes, or in another map-wide record/routine. At `0x47b87e`, the
runtime reads cell byte `+0x00`, uses it as a layer index, and re-fetches the
same `(x,y)` cell from that layer before checking navigation flags. The upper
byte at `+0x01` remains unresolved.

### 2.5 Object/reference ID is context-sensitive

The high 16 bits at cell `+0x02` are not always “draw this static object now.”
Observed uses include:

- Ordinary scenery cells: reference `mapElem` group `002` artwork.
- Purchasable land (`event class 1`): IDs `000–227` belong to four colour/state
  families used by the runtime property/building system. The initial map does
  not necessarily draw the referenced building.
- Animation trigger (`event class 15`): the value selects a `mapElem` group
  `004` animation/composite definition, not a group-`002` static image. For
  Dinosaur `(19,25)`, object ID `1` selects `004/001`, whose frames begin at
  `003/000`. Dinosaur `(15,22)`, object ID `2`, selects `004/002`, whose frames
  begin at `003/046`.
- `MB1.bmp`/object ID `0` occurs on many cells without a visibly drawn MB1
  object. This is not enough to instantiate artwork: the executable also checks
  the low three terrain/control bits. Mode `7` creates a static map drawable,
  while mode `6` creates another interactive entity. Mode `0` cells such as the
  reported MB1 cells do not enter either creation branch. MB1's higher-level
  authoring meaning remains unknown, but its non-rendering is now confirmed.

Therefore a catalogue can show the associated filename, but a renderer must not
blindly paint every cell's group-`002` image.

### 2.6 Terrain ID and terrain flags

The terrain ID selects a group-`000` `80 × 60` terrain tile. Different IDs
visually encode different path connection/orientation artwork. This is not the
same as proving that the numeric ID itself is a movement-direction bitmask.

The low byte of the terrain/control word is actively decoded by the executable:

| Bit(s) | Confirmed low-level use                                                                                                  |
| ------ | ------------------------------------------------------------------------------------------------------------------------ |
| `0x80` | Required by the normal navigation-cell query at `0x47b8c3`; without it the query rejects the cell.                       |
| `0x40` | Rejected by the same query at `0x47b8d4`; this is a normal-navigation block bit.                                         |
| `0x38` | Runtime state bits. The loader ORs them into every cell at `0x478165–0x47817b`; they are not separate stored fields.     |
| `0x07` | Entity/terrain mode. The loader dispatches on this mask; mode `7` creates a drawable and mode `6` an interactive entity. |

In abbreviated x86, the normal navigation test is:

```asm
47b8c3  movsx edx, byte ptr [cell+4]
47b8c7  and   edx, 80h       ; must be present
47b8d4  movsx ecx, byte ptr [cell+4]
47b8d8  and   ecx, 40h       ; must be absent
```

This function also rejects an occupied runtime slot. It is therefore best
described as the game's **normal free-navigation query**, not proof that every
`0x40` cell can never participate in special event or layer logic.

Observed stored combinations can now be decomposed as follows:

| Flags    | Evidence-based description                                                                                       |
| -------- | ---------------------------------------------------------------------------------------------------------------- |
| `0x0080` | Navigation enabled, mode `0`.                                                                                    |
| `0x0085` | Navigation enabled, mode `5`; common on event cells, but mode `5`'s additional behaviour remains unresolved.     |
| `0x00c0` | Enable + block, mode `0`; rejected by the normal free-navigation query. It is not specifically “purchasable.”    |
| `0x00c5` | Enable + block, mode `5`; rejected normally. Its eight-map jail-boundary correlation remains a strong candidate. |
| `0x00c6` | Enable + block, mode `6`; loader creates an interactive entity and specially links event classes `6` and `15`.   |
| `0x00c7` | Enable + block, mode `7`; loader creates a drawable using the cell's object/reference ID.                        |

Snow-map examples demonstrate the split:

```text
cell (16,24), word +0x04 = 0x00020080 -> terrain 2, flags 0x0080
cell (18,21), word +0x04 = 0x00000080 -> terrain 0, flags 0x0080
cell (18,23), word +0x04 = 0x00080085 -> terrain 8, flags 0x0085
cell (18,24), word +0x04 = 0x000b0080 -> terrain 11, flags 0x0080
```

Terrain IDs `0`, `2`, `8`, and `11` select different visual path shapes. No
separate, confirmed per-cell “north/east/south/west” field has been found.

### 2.7 Price/raw value

For event class `1`, cell `+0x08` is **confirmed as the purchase price** from
gameplay evidence and global correlation:

- All 497 class-1 cells across the eight maps have a non-zero value.
- Nearly every non-zero value belongs to class 1.
- Two Tree-map class-0 cells contain value `10`; because their role is unknown,
  the field must still be displayed as “raw value” outside class 1.

It must not be labelled “route.”

### 2.8 Event class

Current event-class table:

|    Value | Meaning               | Confidence/source                                    |
| -------: | --------------------- | ---------------------------------------------------- |
|      `0` | None                  | Confirmed by correlation                             |
|      `1` | Purchasable land      | Confirmed by gameplay and prices                     |
| `2`, `3` | Unknown               | No label                                             |
|      `4` | Mini-game             | Confirmed from gameplay/map placement                |
|      `5` | Shop                  | Confirmed; all record-`003` coordinates land here    |
|      `6` | Bank                  | Confirmed from gameplay knowledge                    |
|      `7` | Extra/double turn     | Confirmed from gameplay knowledge                    |
|      `8` | Bomb                  | Confirmed from gameplay knowledge                    |
|      `9` | Hole/teleport to jail | Confirmed event; destination linkage is map-wide     |
|     `10` | Bonus                 | Confirmed from gameplay knowledge                    |
|     `11` | Penalty               | Confirmed; only a subset forms the slowing jail path |
|     `12` | Penalty-pool payout   | Confirmed from gameplay knowledge                    |
|     `13` | Big event             | User-observed candidate; exact behaviour unverified  |
|     `14` | Hermit event          | Confirmed from gameplay knowledge                    |
|     `15` | Animation trigger     | Confirmed by group-`004` frame links                 |

The class is an event type, not a geometric “region.” Class 11 alone does not
mean jail: class-11 cells outside the jail still impose a monetary penalty but
do not necessarily slow movement.

No event-class-`2` or event-class-`3` cells exist in any of the eight original
maps. All `28,000` cells were checked. These values are therefore reserved or
unused in the shipped map set; assigning names to them would currently be
speculation.

Executable proof: the landing dispatcher at `0x4441b8` reads the signed byte at
`cell +0x0c` directly. It subtracts `4` and jump-dispatches classes `4–15`.
Classes `4`, `5`, `6`, and `15` have dedicated front-end branches; classes
`7–14` funnel into a shared event path, consistent with their individual effects
being selected by the game's event data/controller rather than by this switch.
Another routine at `0x4410c0` passes nonzero classes other than `1` and `6` into
the generic event state. This confirms the field and dispatch range, but does
not by itself prove the gameplay names for `7–14`.

### 2.9 Trailing BMP path catalogue

The cell array ends at:

```text
pathsOffset = 0x50 + width * height * 0x10
```

After that offset is a NUL-terminated catalogue of original authoring paths.
The first path names the map-selection artwork. Every subsequent path aligns
exactly with a group-`002` element ID:

```text
path[0]     = map preview/source artwork label
path[1 + n] = name for mapElem group 002 entry n
```

For Doraemon Town, path zero is:

```text
K:\G9802PC - Doraemon\ARTWORK\GAME\Map\Doraemon_Town\Map_Block_Doraemon.bmp
```

That path is embedded metadata from the original developer's machine. Resource
Studio did not invent or obtain that BMP from the current filesystem.

## 3. Record `001`: PCX preview

Record `001` is a complete embedded 8-bit indexed PCX image used as the map
selection/source-art preview. Its PCX header, indexed pixels, and trailing
palette are decoded normally. It is independent of the group-`005` map-element
palette, although the artwork may visually match the map.

## 4. Record `002`: 280-byte map-wide configuration

Record `002` is always exactly `0x118` bytes in all eight originals: 70
little-endian `u32` words. It is not a cell array.

### 4.1 Confirmed and strong-candidate fields

| Offset          | Type                                 | Meaning                                                                                                         |
| --------------- | ------------------------------------ | --------------------------------------------------------------------------------------------------------------- |
| `+0x00`         | `u32`                                | **Confirmed map ID**, equal to `NNNN`.                                                                          |
| `+0x04..+0x30`  | four `{i32 x, i32 y, i32 z}` triples | **Confirmed four player start positions.**                                                                      |
| `+0x34..+0x40`  | `u32[4]`                             | **Confirmed four start direction codes**, one per player. The executable movement enum is `0=N, 1=S, 2=W, 3=E`. |
| `+0x44`         | `i32`                                | **Strong candidate jail destination/entry X.** Every hole uses the same map-wide destination.                   |
| `+0x48`         | `i32`                                | **Strong candidate jail destination/entry Y.**                                                                  |
| `+0x4c`         | `u32`                                | **Strong candidate jail orientation/direction code.** Observed values are `1` and `3`.                          |
| `+0x50..+0x113` | `u32[49]`                            | Mixed raw configuration. Several runs look like coordinate triples, but most semantics are unresolved.          |
| `+0x114`        | `u32`                                | Unknown map-wide scalar. Values are 5, 10, or 20.                                                               |

The jail geometry correlation is currently:

- Direction code `3`: the jail approach extends along positive X toward the
  destination. The bomb is four cells before it.
- Direction code `1`: the jail approach extends along positive Y toward the
  destination. The bomb is four cells before it.
- The three cells between bomb and destination are ordinary approach cells.
- Across all maps, the derived bomb cell has event class `8` and the destination
  has event class `11`, strongly supporting this interpretation.
- The direction probably ensures a hole sends a player into the jail facing away
  from the anti-backtracking bomb. The exact hole-transfer routine remains to be
  traced.

The record-`002` getter at `0x47f402` returns these four direction words without
translation. The movement routines use the same numeric enum: `0` decrements Y,
`1` increments Y, `2` decrements X, and `3` increments X. This corrects the
earlier gameplay-derived labels that had codes `1` and `3` swapped.

For direction code `3`:

```text
bomb     = (jailX - 4, jailY)
approach = (jailX - 3, jailY), (jailX - 2, jailY), (jailX - 1, jailY)
```

For direction code `1`:

```text
bomb     = (jailX, jailY - 4)
approach = (jailX, jailY - 3), (jailX, jailY - 2), (jailX, jailY - 1)
```

The resulting cross-map correlation is:

| Map                 | Candidate jail destination | Direction | Derived bomb | Destination class | Bomb class |
| ------------------- | -------------------------: | --------: | -----------: | ----------------: | ---------: |
| Dinosaur (`0`)      |                  `(30,29)` |       `3` |    `(26,29)` |              `11` |        `8` |
| Ice/Snow (`1`)      |                  `(15,12)` |       `3` |    `(11,12)` |              `11` |        `8` |
| Tree (`2`)          |                  `(19,27)` |       `3` |    `(15,27)` |              `11` |        `8` |
| Western (`3`)       |                  `(12,30)` |       `1` |    `(12,26)` |              `11` |        `8` |
| Doraemon Town (`4`) |                  `(18,18)` |       `3` |    `(14,18)` |              `11` |        `8` |
| Future (`5`)        |                  `(11,27)` |       `1` |    `(11,23)` |              `11` |        `8` |
| All map (`6`)       |                  `(41,38)` |       `3` |    `(37,38)` |              `11` |        `8` |
| MMap02 (`7`)        |                  `(52,33)` |       `1` |    `(52,29)` |              `11` |        `8` |

### 4.2 Full Doraemon Town record `002`

`map0004.dat` contains:

```text
+000 00000004   map ID 4
+004 00000018   player 1 X = 24
+008 00000019   player 1 Y = 25
+00c 00000000   player 1 Z = 0
+010 00000018   player 2 X = 24
+014 00000018   player 2 Y = 24
+018 00000000   player 2 Z = 0
+01c 00000019   player 3 X = 25
+020 00000019   player 3 Y = 25
+024 00000000   player 3 Z = 0
+028 00000019   player 4 X = 25
+02c 00000018   player 4 Y = 24
+030 00000000   player 4 Z = 0
+034 00000002   player 1 direction code = 2
+038 00000000   player 2 direction code = 0
+03c 00000001   player 3 direction code = 1
+040 00000003   player 4 direction code = 3
+044 00000012   jail destination X = 18 (strong candidate)
+048 00000012   jail destination Y = 18 (strong candidate)
+04c 00000003   jail direction code = 3 (strong candidate)
+050 00000000   raw
+054 00000000   raw
+058 00000000   raw
+05c 00000000   raw
+060 00000000   raw
+064 00000000   raw
+068 00000000   coordinate-shaped raw X = 0
+06c 00000018   coordinate-shaped raw Y = 24
+070 00000000   coordinate-shaped raw Z = 0
+074 00000031   coordinate-shaped raw X = 49
+078 00000018   coordinate-shaped raw Y = 24
+07c 00000000   coordinate-shaped raw Z = 0
+080 00000000   raw
+084 00000000   raw
+088 00000000   raw
+08c 00000000   raw
+090 00000000   raw
+094 00000000   raw
+098 00000011   raw (17)
+09c 00000012   raw (18)
+0a0 00000000   raw
+0a4 00000013   raw (19)
+0a8 00000014   raw (20)
+0ac 00000000   raw
+0b0 0000000e   coordinate-shaped raw (14,17,0)
+0b4 00000011
+0b8 00000000
+0bc 00000017   coordinate-shaped raw (23,19,0)
+0c0 00000013
+0c4 00000000
+0c8 00000000   raw
+0cc 00000000   raw
+0d0 00000000   raw
+0d4 00000000   raw
+0d8 00000000   raw
+0dc 00000000   raw
+0e0 00000000   raw
+0e4 00000000   raw
+0e8 00000000   raw
+0ec 00000000   raw
+0f0 00000000   raw
+0f4 00000000   raw
+0f8 00000000   raw
+0fc 00000000   raw
+100 00000000   raw
+104 00000000   raw
+108 00000000   raw
+10c 00000000   raw
+110 00000000   raw
+114 0000000a   unknown scalar = 10
```

The values at `+0x68..+0x7c` look like two points on the central horizontal
line, `(0,24,0)` and `(49,24,0)`, but their role is not yet proven. Similar
coordinate-shaped blocks occur elsewhere. A value fitting inside map bounds is
not sufficient evidence that it is a gameplay coordinate.

### 4.3 Snow penguin patrol candidate

Snow/Ice (`map0001.dat`) has a unique non-zero block:

```text
+0xe8 = 0x00000001   likely enabled/count
+0xec = 0x0000000a   10
+0xf0 = 0x0000000a   10
+0xf4 = 0x00000000   0
+0xf8 = 0x0000001f   31
+0xfc = 0x0000001a   26
+0x100 = 0x00000000  0
+0x104 = 0x00000000  0
```

The two triples `(10,10,0)` and `(31,26,0)` match opposite corners of the
observed penguin's rectangular patrol, so this is a **strong patrol-bounds
candidate**. The four corners are obtained by combining the two X values with
the two Y values. This block still does not contain an identified penguin image
ID or current-facing direction. Selection of the actor/animation may be implicit
in the block's fixed offset or hard-coded in `Doraemon.exe`.

The executable provides one additional behavioural clue: a moving-actor routine
at `0x49697a` calls the map direction builder `0x47e833` for direction codes
`0–3`, combines the returned connection bits, and moves using computed map
topology. This supports a runtime-computed patrol direction. The remaining link
from this actor routine to the Snow `+0xe8` block, and the source of the penguin
pixels, still needs a direct data-flow trace before it can be called confirmed.

The penguin pixels are not in Snow's map-local group `003`: those entries are
mine/cave animation frames and one event tile. `anime.dat` stores animation
definitions and frame references rather than pixels. A precise actor link is
still unresolved.

## 5. Record `003`: shops/special locations

Record `003` begins with a count and then fixed-size entries:

```text
+0x00  u32  entry count
+0x04  entry 0
        entry 1
        ...
```

Each entry is exactly `0x15c` (348) bytes or 87 `u32` words:

| Entry offset    | Type      | Meaning                                                     |
| --------------- | --------- | ----------------------------------------------------------- |
| `+0x00`         | `i32`     | **Confirmed X coordinate.**                                 |
| `+0x04`         | `i32`     | **Confirmed Y coordinate.**                                 |
| `+0x08`         | `i32`     | **Confirmed Z coordinate.**                                 |
| `+0x0c..+0x15b` | `u32[84]` | Exact shop parameter table; individual meanings unverified. |

```text
entryOffset(i) = 0x04 + i * 0x15c
```

All 23 entries across the eight maps point to event-class-5 cells and to the
`Zshop1`/`Zshop2` artwork family. This confirms that these are shop records, not
generic regions. We preserve all 84 parameter words and their offsets because
they may contain inventory, prices, dialogue, or behaviour controls, but those
labels require executable or gameplay evidence.

Record lengths agree exactly with the formula:

```text
length = 4 + count * 348
```

For example, three shops produce `4 + 3*348 = 1048` bytes.

Counts in map order `0–7` are `3, 3, 2, 3, 2, 4, 3, 3`, totalling 23
records. This count agreement is also useful for detecting a corrupt or
incorrectly sliced record.

## 6. Related `mapElemNNNN.dat` archive

The map archive contains layout/configuration. The paired map-element archive
contains indexed artwork and animation definitions:

| Group | Interpretation                                                                                                                                |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `000` | `80 × 60` terrain tiles using palette index `255` as a colour-keyed transparent background. Header record-`000` `+0x30` is their exact count. |
| `001` | Ten raw opaque `80 × 60` indexed tiles.                                                                                                       |
| `002` | Static/runtime objects and building variants, decoded with indexed RLE. Original BMP names come from map record `000`.                        |
| `003` | Animation frames/overlays, decoded with indexed RLE.                                                                                          |
| `004` | Animation/composite definitions that reference group-`003` frame IDs.                                                                         |
| `005` | The map's 256-colour VGA palette.                                                                                                             |

Group `002` entries `000–227` are byte-for-byte identical across all eight maps
and form runtime property/building colour/state families. Entries through `274`
are also identical, `275–277` have a small ordering difference, `278–297` are
shared, and `298+` contain genuinely map-specific scenery.

## 7. Direction information: what exists and what does not

Confirmed stored direction-related values are:

- Four player start direction codes in record `002` at `+0x34..+0x40`.
- One jail direction/orientation code at `+0x4c` (strongly correlated).
- Ordered frame references in group-`004` animation definitions.

Not stored as independent metadata:

- No universal facing/direction field exists in sprite pixel records.
- There is no dedicated per-cell four-way direction word. The executable builds
  a four-bit availability mask by probing neighbouring cells at runtime.
- Terrain ID selects visual connection/orientation variants, but the executable
  movement routine does not decode the terrain ID as a NESW mask.
- The Snow patrol block gives bounds, not an explicit current direction.

At `0x47edd1`, a simple neighbour probe builds this mask:

| Mask bit | Neighbour | Movement enum |
| -------: | --------- | ------------: |
|   `0x01` | north     |           `0` |
|   `0x02` | south     |           `1` |
|   `0x04` | west      |           `2` |
|   `0x08` | east      |           `3` |

The more complete routine at `0x47e833` checks immediate neighbours and
look-ahead continuity, removes an immediate reverse where appropriate, and
returns the same four-bit mask. Callers randomly choose among the remaining set
bits. A moving-actor routine at `0x49697a` also calls `0x47e833` for all four
directions, showing that direction is computed for map actors as well as
players rather than stored in each sprite record.

### 7.1 Doraemon Town start-junction evidence

Executable coordinate updates establish this direction-code mapping:

| Code |   Vector | Direction |
| ---: | -------: | --------- |
|  `0` | `(0,-1)` | north     |
|  `1` | `(0,+1)` | south     |
|  `2` | `(-1,0)` | west      |
|  `3` | `(+1,0)` | east      |

Player 2 starts at `(24,24)` heading north. Its exact cell is:

```text
00 00 ff ff 80 00 15 00 00 00 00 00 00 00 00 00
                 -----
                 terrain ID 0x0015 = 21, flags 0x0080
```

At this junction the game can choose north, east, or west. South is the reverse
of the player's current heading and is excluded. The next cell `(24,23)` is:

```text
00 00 ff ff 80 00 0e 00 00 00 00 00 00 00 00 00
                 -----
                 terrain ID 0x000e = 14, flags 0x0080
```

At `(24,23)` only forward/north is offered. Although its east neighbour has the
basic `0x80` navigation-enable bit, the executable's look-ahead test can reject
that branch because it has no continuation other than returning to the current
cell. This is stronger evidence than the earlier terrain-ID hypothesis.

The corrected movement model is:

1. Probe the four neighbouring logical cells through the normal navigation
   query, including effective-layer, flag, and runtime-occupancy checks.
2. Apply look-ahead/dead-end checks and remove an immediate reverse.
3. Encode surviving exits as bits `1,2,4,8`; if several remain, the caller may
   choose randomly according to its current movement state.

Terrain `21` visually depicts a junction and terrain `14` a straight route, so
the artwork agrees with the authored topology, but the terrain pixels and ID do
not control it directly. The terrain sprite record contains image fields, row
offsets, and indexed pixels only.

### 7.2 Doraemon Town terrain catalogue arrangement

The 64 Doraemon Town group-`000` records are arranged in artwork families, not
as one obvious numeric NESW bitmask:

| IDs     | Visible family                                                                 |
| ------- | ------------------------------------------------------------------------------ |
| `0–15`  | Ordinary road/path edge, straight, and rotated corner variants                 |
| `16–19` | Larger junction/corner combinations                                            |
| `20–23` | Four centre/start variants; their placement aligns with the four player starts |
| `24–29` | Multi-edge junction and crossing variants                                      |
| `30–31` | Plain/base diamonds                                                            |
| `32–35` | Four purchasable-land variants with the dorayaki marker                        |
| `36–39` | Plain edge/notch variants                                                      |
| `40–47` | Road and pedestrian-crossing combinations/rotations                            |
| `48–55` | Painted text or marked event/path variants                                     |
| `56`    | Distinct yellow patterned tile                                                 |
| `57–63` | Mostly empty/outline-only terrain records                                      |

Sequential IDs often contain rotations of a common design, but the family sizes
and ordering are not uniform enough to decode direction as `terrainId & mask`.
For movement analysis, each ID should receive an evidence-derived connection
mask from its artwork and repeated placement, then be checked against executable
logic.

## 8. Rules for future decoding

1. Always retain the original bytes and source offset.
2. Split cell words into high/low 16-bit fields before assigning meaning.
3. Treat object IDs according to event context; do not render every ID as static
   scenery.
4. Label `+0x08` as price only when class `1`; otherwise show “raw value.”
5. Label record-`003` entries as shops, but keep their 84 parameters raw.
6. Use the executable direction enum `0=N, 1=S, 2=W, 3=E`, but keep jail slowing
   boundaries, patrol actor IDs, mode-`5` behaviour, and unknown header/config
   fields marked unverified until traced to executable behaviour or demonstrated
   by controlled gameplay edits.
