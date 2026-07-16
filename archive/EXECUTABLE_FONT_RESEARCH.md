# Doraemon Monopoly executable font research

This document records verified behavior in the Chinese Windows 95/98 executable used by this repository. Addresses are virtual addresses for the known executable whose SHA-256 is `fdf00e681671f93b09d257f77d7ce0720e7129cf6bc44ba9e0f19c2efa4fecba`.

## Font files and globals

The dedicated font subsystem starts at `0x004D1000`.

| Address      | Purpose                                                      |
| ------------ | ------------------------------------------------------------ |
| `0x004D0002` | Offset-table pointer for the active 128-slot sysfont variant |
| `0x004D0006` | Base address of the loaded `sysfont.dat`                     |
| `0x004D000A` | `sysfont.dat` filename                                       |
| `0x004D001A` | Active sysfont variant number                                |
| `0x004D001E` | Base address of the loaded `chifont.dat`                     |
| `0x004D0022` | `chifont.dat` filename                                       |
| `0x004D0032` | Height of the active sysfont variant                         |

The loader reads each complete file into allocated memory. It does not contain a fixed 640-glyph sysfont allocation. `0x004D113F` selects a variant by multiplying its number by 128 and setting the active offset-table pointer.

## Original encoding

- Bytes below `0x80` select a glyph from the active 128-slot sysfont variant.
- Bytes at or above `0x80` consume a second byte.
- A Chinese ID is `((first & 0x7f) << 8) | second`.
- Chinese glyph data starts at `chifontBase + ID * 32`.
- Every Chinese glyph is a headerless 16×16 one-bit bitmap and advances exactly 16 pixels.

The original Chinese archive contains glyph IDs 0–740. It contains no glyph whose first byte is `0xCC` or `0xCD`.

## Rendering functions

| Function     | Behavior                                     |
| ------------ | -------------------------------------------- |
| `0x004D118E` | Measures a complete NUL-terminated string    |
| `0x004D11E2` | Counts encoded string bytes                  |
| `0x004D120C` | Measures one encoded character               |
| `0x004D123C` | Builds a bitmap object for one character     |
| `0x004D1364` | Builds a bitmap object for a complete string |

The high-bit branches suitable for interception are:

| Address      | Patch purpose              |
| ------------ | -------------------------- |
| `0x004D11D0` | Complete-string width      |
| `0x004D1235` | Single-character width     |
| `0x004D12E1` | Single-character rendering |
| `0x004D1444` | Complete-string rendering  |

The original Chinese instructions overwritten at the rendering branch entries must be reproduced before returning to `0x004D12E8` or `0x004D144C`.

## Vietnamese extension

The prototype reserves two unused high-bit prefixes:

- `CC 00`–`CC 7F`: Vietnamese slots 0–127.
- `CD 00`–`CD 7F`: Vietnamese slots 128–255.

The expanded sysfont has 1,920 records:

```text
0..639       original five 128-slot variants
640..895     Vietnamese bank for active variant 0
896..1151    Vietnamese bank for active variant 1
1152..1407   Vietnamese bank for active variant 2
1408..1663   Vietnamese bank for active variant 3
1664..1919   Vietnamese bank for active variant 4
```

The patched record index is:

```text
640 + activeVariant * 256 + VietnameseSlot
```

### Mapping order and examples

`resource-studio/src/lib/vietnamese-font.ts` is the mapping's single source of
truth. It constructs a stable 134-character array in this order:

1. lowercase `a ă â e ê i o ô ơ u ư y`, each ordered as unmarked,
   grave, acute, hook, tilde, dot;
2. lowercase `đ`;
3. the same uppercase vowel sequence and tone order;
4. uppercase `Đ`.

Only non-ASCII results receive slots. For a character with slot `S`:

```text
S < 128:  bytes = CC S
S >= 128: bytes = CD (S - 128)
record index = 640 + activeVariant * 256 + S
```

| Character | Slot | Encoded bytes | Variant 0 | Variant 1 | Variant 2 | Variant 3 | Variant 4 |
| --------- | ---: | ------------- | --------: | --------: | --------: | --------: | --------: |
| `á`       |    1 | `CC 01`       |       641 |       897 |      1153 |      1409 |      1665 |
| `ă`       |    5 | `CC 05`       |       645 |       901 |      1157 |      1413 |      1669 |
| `ú`       |   51 | `CC 33`       |       691 |       947 |      1203 |      1459 |      1715 |
| `đ`       |   66 | `CC 42`       |       706 |       962 |      1218 |      1474 |      1730 |
| `Đ`       |  133 | `CD 05`       |       773 |      1029 |      1285 |      1541 |      1797 |

Font Studio displays all three identifiers on every Vietnamese card: absolute
record index, character, and encoded byte pair with logical slot.

All other high-bit pairs retain the original Chinese behavior. A second byte with bit 7 set is deliberately rejected by the Vietnamese dispatch and falls back to the Chinese path.

## Patch space and PE constraints

`CSEG` has RVA `0xD1000`, raw offset `0xCC000`, raw size `0x2000`, and original virtual size `0x1BD5`. Bytes from virtual address `0x004D2C00` through `0x004D2FFF` provide a 1,024-byte code cave.

The patcher:

1. validates the exact executable SHA-256;
2. the early prototype replaced the filename at `0x004D000A` with
   `sysfont-vi.dat`; the current Rust patch leaves the original `sysfont.dat`
   literal unchanged;
3. writes dispatch stubs into that cave;
4. rewrites the four branch entries as near jumps;
5. increases CSEG virtual size to `0x2000`;
6. adds the executable section characteristic.

The executable has stripped relocations and a fixed image base, so the prototype uses the verified absolute addresses above. Another regional or patched executable must be analyzed and fingerprinted separately.

## Runtime verification still required

Static analysis confirms that the dispatch and proportional lookup are feasible. DOSBox-X testing must still verify:

- the patched CSEG executes on the target Windows installation;
- variant switches select the intended Vietnamese bank;
- bitmap ownership and cleanup remain correct for both rendering entry points;
- mixed Vietnamese, ASCII, and Chinese strings render without corruption.

Place a debugger breakpoint on `0x004D113F` to observe runtime variant changes.
