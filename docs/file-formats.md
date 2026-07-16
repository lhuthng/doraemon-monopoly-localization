# Doraemon Monopoly (Windows 95/98) — reverse-engineering notes

This repository investigates the resource architecture used by the Cantonese
and Taiwanese releases of GameOne's 1998 _Doraemon Monopoly_. It distributes no
game executable or resource archive. The Studio accepts user-supplied files and
may auto-load ignored private copies from `resource-studio/public/game/` during
local development.

The findings below come from static inspection of the supplied installations,
comparison between releases, visual rendering, controlled file-removal and
glyph-patching tests, and reimplementing the game's decompressor.

## Confidence labels

- **Confirmed**: validated against every relevant record, visually matched to
  game output, or verified by a decode/re-encode round trip.
- **Strong inference**: the structure is decoded consistently and the proposed
  meaning fits the observed behavior, but the executable call site has not yet
  been traced completely.
- **Unknown**: the file or field has only been identified partially.

## Implementations

The current implementations are:

- `resource-studio/src/lib/formats.ts` — GameOne archive, LZW codec, string
  records, `sysfont.dat`, and archive reconstruction
- `resource-studio/src/lib/asset-formats.ts` — PCX bitmap and transparent sprite
  decoding

## Known-file index

| File/family                        | Responsibility                        | Knowledge                      | Specification                                               |
| ---------------------------------- | ------------------------------------- | ------------------------------ | ----------------------------------------------------------- |
| GameOne container                  | Recursive owner of compressed records | Confirmed                      | [Open](#gameone-archive-container)                          |
| `strings-CN.dat`, `strings-TW.dat` | Game text records                     | Confirmed                      | [Open](#strings-cndat-and-strings-twdat)                    |
| `chifont.dat`                      | Indexed Chinese glyphs                | Confirmed                      | [Open](#chifontdat)                                         |
| `sysfont.dat`                      | Proportional single-byte fonts        | Confirmed                      | [Open](#sysfontdat)                                         |
| `Fonts.dat`                        | Secondary font records                | Outer confirmed; inner unknown | [Open](#fontsdat)                                           |
| `bitmaps.dat`                      | Complete screens/background artwork   | Confirmed PCX subset           | [Open](#bitmapsdat-embedded-pcx-screens)                    |
| `Sprite1.dat`                      | Hotspot-anchored transparent sprites  | Confirmed                      | [Open](#sprite1dat-and-sprite2dat-indexed-scanline-sprites) |
| `sprite2.dat`                      | Top-left-anchored indexed UI sprites  | Confirmed                      | [Open](#sprite1dat-and-sprite2dat-indexed-scanline-sprites) |
| `interface.dat`                    | Probable UI composition data          | Strong inference               | [Open](#interfacedat-and-composition)                       |
| `voice-CN.dat`, `voice-TW.dat`     | Spoken dialogue archives              | Confirmed WAVE leaves          | [Open](#voice-cndat-and-voice-twdat)                        |
| `Sfx.dat`                          | Sound effects                         | Unknown                        | [Open](#other-retained-resources)                           |
| `map*.dat`, `mapElem*.dat`         | Board and board elements              | Purpose inferred               | [Open](#other-retained-resources)                           |
| `MGame*.DAT`, `MiniGame.DAT`       | Minigame configuration                | Purpose inferred               | [Open](#other-retained-resources)                           |
| Gameplay `.dat` files              | Animation, buildings, events, gadgets | Purpose inferred               | [Open](#other-retained-resources)                           |
| `Databaseaeb8.dat`                 | Stateful game/save database           | Strong inference               | [Open](#other-retained-resources)                           |

## GameOne archive container

Many `.dat` files share the same proprietary container. A container begins
with the following signature:

```text
00 00 "GameOne Systems Limited\nWritten by Samme NG" 00
```

All integers described here are little-endian.

| Container-relative offset | Meaning                                                |
| ------------------------: | ------------------------------------------------------ |
|                    `0x42` | `u32` child-entry count                                |
|                    `0x66` | beginning of an array of `count` × `u32` child offsets |

Each offset is relative to the beginning of its current container, not the
beginning of the whole file. A child can begin with another GameOne signature,
making the archive recursive. A leaf ends where the next known node begins, or
at end-of-file for the final leaf.

The regional strings files demonstrate the nesting clearly: each outer archive contains
nine child archives with `36, 42, 51, 136, 136, 136, 136, 136, 136` leaves,
for a total of 945 records. A stable record ID is therefore its archive path,
for example `003/027`; it is not a byte offset.

To obtain actual data, recursively walk the table, slice each leaf at the next
known node boundary, then apply the decoder below. Preserve paths such as
`003/027`; physical offsets move when an archive is rebuilt.

### Rebuilding a container

The implemented writer reconstructs from the leaves upward:

1. Preserve the original bytes before the first child, including unknown
   header fields.
2. Replace selected leaf payloads; preserve all other leaf payloads verbatim.
3. Recursively rebuild child containers.
4. Concatenate the rebuilt children.
5. Rewrite the offset table at `0x66` using each child's new
   container-relative position.
6. Parse the completed archive again and verify every expected leaf.

This strategy avoids pretending that currently unknown header bytes are safe
to regenerate.

## Leaf compression: fixed 14-bit LZW family

GameOne archive leaves used by the investigated resources are commonly wrapped
in the same codec. The executable decoder was identified around virtual address
`0x00409247`, then reconstructed and checked against all 945 strings and the
asset archives.

### Compressed payload layout

```text
+0x00  u32  expected decompressed byte length
+0x04  ...  MSB-first stream of fixed-width 14-bit codes
```

Codec rules:

- Literal byte codes: `0x0000`–`0x00ff`
- First dictionary code: `0x0100`
- End-of-stream code: `0x3fff`
- Largest dictionary entry: `0x3ffe`
- Code width: always 14 bits; it does not grow dynamically
- Bit order: most-significant bit first
- Dictionary entries are `(previous code, appended first byte)` pairs
- The normal LZW “code equals next dictionary index” special case is supported
- Decoded size must equal the leading `u32`

### Encoding

The encoder starts with all 256 one-byte phrases, grows the dictionary from
`0x0100`, emits the longest known phrase, stops adding entries after `0x3ffe`,
appends `0x3fff`, and packs every code MSB-first using exactly 14 bits.

Every newly compressed record is immediately decompressed in memory and
byte-compared with its source before it is allowed into an archive. After
archive reconstruction, the entire archive is parsed once more. This caught
several early offset and preservation mistakes.

## `strings-CN.dat` and `strings-TW.dat`

After LZW decompression, each string follows this byte grammar:

- `0x00`: terminator
- byte `< 0x80`: single-byte character/control byte
- byte `>= 0x80`: first half of a two-byte Chinese glyph reference
- literal ASCII `\N` or `\n`: game newline marker

The two-byte glyph ID is:

```text
glyph_id = ((first_byte & 0x7f) << 8) | second_byte
```

For example, the byte `0x80` does **not** mean decimal glyph 128 by itself. It
contributes the high seven bits; the following byte completes the ID. Thus
`80 8d` addresses glyph `0x008d`, decimal 141.

All 945 original records validate under this grammar. Together they reference
715 distinct Chinese glyph IDs in the range `0…740`. The file contains ordinary
dialogue/UI text and short execution/status messages. Apparent “missing words”
in some records are usually values inserted by game logic at runtime (numbers,
player names, item quantities, and similar state), rather than an undiscovered
placeholder byte inside the record.

The exact formatting-call convention for every dynamic insertion is still
unknown. Absence of a visible `%d`-style marker is therefore not proof that a
record is displayed literally.

## `chifont.dat`

| Property        |        Value |
| --------------- | -----------: |
| File size       | 23,904 bytes |
| Bytes per glyph |           32 |
| Glyph count     |          747 |
| Dimensions      | 16×16 pixels |

Each glyph is 32 bytes, interpreted as 16 rows × 16 bits. A set bit is a drawn
pixel. There is no header or Unicode table; record position is the glyph ID.

```text
glyph_offset = glyph_id * 32
row_bits     = big-endian 16-bit value at glyph_offset + row * 2
```

Rendering the atlas made the first decisive text breakthrough: glyphs could be
read visually, and sequences from `strings.dat` formed sensible Chinese
sentences. Cross-checking repeated glyph IDs across many sentences produced the
glyph-ID-to-character map.

## `sysfont.dat`

`sysfont.dat` is an indexed proportional bitmap-font file, not a GameOne
archive.

| Offset | Type       | Meaning                       |
| -----: | ---------- | ----------------------------- |
| `0x00` | `u16`      | Glyph count: 640              |
| `0x02` | `u32[640]` | Absolute glyph-record offsets |

Each glyph record is:

| Glyph offset | Type                 | Meaning              |
| -----------: | -------------------- | -------------------- |
|       `0x00` | `u8`                 | Width                |
|       `0x01` | `u8`                 | Height               |
|       `0x02` | `u8[width × height]` | Pixel/intensity data |

The count is `5 × 128`, strongly matching five visual variants of the
single-byte `0x00…0x7f` character set. Glyph `variant * 128 + ascii_byte`
selects a variant. Width is per glyph, so Latin text is not necessarily fixed
at 8 or 16 pixels; the game can advance by each glyph's stored width.

This explains why text wrapping must be measured in pixels rather than character
count. Chinese glyphs are treated as 16 pixels wide by the reconstructed layout
tool; Latin widths come from the selected `sysfont.dat` variant. The exact
runtime choice of variant for every UI panel remains partly inferred from
screenshots.

There are only 128 slots per variant. `sysfont.dat` does not directly expose a
256-character or Unicode table. Extending the file alone would not prove the
executable can address extra slots; that would require patching/tracing the
renderer.

## `Fonts.dat`

`Fonts.dat` is not the same format as `sysfont.dat` and should not be renamed or
substituted for it. It is a GameOne archive containing 2,560 leaves, all of
which pass the 14-bit decompressor. The decoded leaves begin with repeated
structured tables, but their inner record format has not been established.

| Layer/property | Current knowledge                                           |
| -------------- | ----------------------------------------------------------- |
| Outer format   | GameOne container                                           |
| Leaves         | 2,560                                                       |
| Compression    | All leaves pass the 14-bit decoder                          |
| Inner format   | Repeated structured tables; unknown                         |
| Likely role    | Additional font sizes/styles or rendering data; unconfirmed |

Confirmed facts stop there. It may provide additional sizes/styles or
precomputed font-rendering data, but that is currently a hypothesis. The tools
must not expose it as an editable font until the record semantics and runtime
consumer are verified.

## `bitmaps.dat`: embedded PCX screens

`bitmaps.dat` is a GameOne archive. After outer LZW decoding, many leaves begin
with the standard 8-bit PCX signature fields:

```text
0a 05 01 08
```

The implemented PCX reader supports the observed form:

| Offset/location | Type             | Meaning                                   |
| --------------: | ---------------- | ----------------------------------------- |
|          `0x00` | `0a 05 01 08`    | PCX signature fields                      |
|     `0x04–0x0b` | four `u16`       | Minimum/maximum X and Y bounds            |
|          `0x41` | `u8`             | Color planes; observed value 1            |
|          `0x42` | `u16`            | Encoded bytes per scanline                |
|          `0x80` | PCX RLE          | Indexed pixel rows, including row padding |
| final 769 bytes | `0c` + 768 bytes | 256 RGB palette entries                   |

In the Chinese installation, 135 of 174 leaves are recognized as this PCX
form. The remaining archive leaves are retained but not claimed to be images.

Observed examples:

- `#053`: 640×480 title background matching the Doraemon-and-friends title
  screen; foreground logo/“Press Button” elements are separate
- `#062`: complete “choose minigame” screen with Chinese text baked into pixels
- `#063`: complete “new record” screen with text baked into pixels
- `#007`: save/load/SFX/music screen artwork, already drawn in English
- `#051`: “Now Loading” artwork
- `#001…#004`: character portraits
- `#023…#050`: character-selection portraits
- `#116`: alphabet/digit graphic sheet

This is the main clue for menu localization: not every visible word comes from
`strings.dat`. Some menu text is raster artwork inside a PCX screen or sprite.

Encoding modified PCX leaves is not implemented. A future writer must preserve
the observed PCX mode, palette, scanline padding, RLE rules, and GameOne/LZW
wrapper before rebuilding the parent archive.

## `Sprite1.dat` and `sprite2.dat`: indexed scanline sprites

Both archives contain transparent palette-indexed RLE sprites after GameOne/LZW
decompression. The executable confirms that flag bit 1 selects this indexed
RLE representation and flag bit 0 controls whether a hotspot is present.
Neither archive embeds an RGB palette.

### Header variants

`Sprite1.dat` normally uses `0x8003`:

| Offset | Type          | Meaning                        |
| -----: | ------------- | ------------------------------ |
| `0x00` | `u16`         | Flags (`0x8003`)               |
| `0x02` | `u16`         | Width                          |
| `0x04` | `u16`         | Height                         |
| `0x06` | `i16`         | Hotspot/origin X               |
| `0x08` | `i16`         | Hotspot/origin Y               |
| `0x0a` | `u16[height]` | Row offsets relative to `0x0a` |

`sprite2.dat` uses `0x8002`:

| Offset | Type          | Meaning                        |
| -----: | ------------- | ------------------------------ |
| `0x00` | `u16`         | Flags (`0x8002`)               |
| `0x02` | `u16`         | Width                          |
| `0x04` | `u16`         | Height                         |
| `0x06` | `u16[height]` | Row offsets relative to `0x06` |

Sprite2 has no missing origin field: its drawing origin is the image's top-left
corner. Treating bytes `0x06…0x09` as Sprite1 hotspot coordinates shifts every
row lookup four bytes late and produces the false transparent horizontal bands
seen in the first decoder.

### Shared row stream

Each row begins at `header_base + row_offset` and contains a `u16` payload-byte
length followed by signed `i16` commands:

- negative `-N`: advance over `N` transparent destination pixels;
- positive `N`: copy the following `N` palette-index bytes;
- zero: copy and skip nothing; it is not a raw-row mode or terminator.

The executable's blitter advances to the next row using the payload length.
Several large Sprite2 records exceed 65,535 decoded bytes, so stored `u16` row
offsets wrap. The decoder reconstructs monotonic offsets by adding `0x10000`
after each wrap; the encoder writes the corresponding low 16 bits.

Because visible bytes are palette indices, the graphics studio uses a selected
PCX bitmap palette only for display and indexed-PNG export. Rebuilding a sprite
does not alter `bitmaps.dat`. Sprite1 resizing preserves the original hotspot,
which may shift alignment; Sprite2 resizing has no hotspot to update.

Both variants now support indexed-PNG export/import, scanline re-encoding,
GameOne archive reconstruction, and a full reparse/pixel/metadata verification
before download. Untouched archive records are preserved byte-for-byte.

## `interface.dat` and composition

`interface.dat` contains 68 compressed numeric records rather than directly
renderable images or strings. Combined with the separation observed on the
title screen—background in bitmap `#053`, foreground graphics elsewhere—the
best current explanation is that `interface.dat` links bitmaps, sprites,
positions, states, or animation sequences.

| Property              | Current knowledge                                           |
| --------------------- | ----------------------------------------------------------- |
| Outer format          | GameOne container                                           |
| Decoded records       | 68 numeric records                                          |
| Direct images or text | None identified                                             |
| Probable role         | UI state/composition links, positions, or animation control |

This is not fully decoded. Finding menu and map-name graphics should currently
proceed by visually cataloguing `bitmaps.dat` and `Sprite1.dat`, then tracing
candidate indices through `interface.dat` and the executable.

## `voice-CN.dat` and `voice-TW.dat`

| Property         |                   CN |                   TW |
| ---------------- | -------------------: | -------------------: |
| Outer format     |    GameOne container |    GameOne container |
| RIFF/WAVE leaves |                  334 |                  328 |
| Leaf content     | Standard WAVE stream | Standard WAVE stream |

To obtain audio, recursively extract and LZW-decompress the leaves, then retain
payloads beginning with RIFF/WAVE markers as ordinary WAVE files.

## Other retained resources

These files are important enough to preserve, but their binary structures have
not yet been established. Purpose based only on a filename is explicitly an
inference.

| File/family                         | Probable responsibility      | Inner format | What is required next                                                |
| ----------------------------------- | ---------------------------- | ------------ | -------------------------------------------------------------------- |
| `Sfx.dat`                           | Sound effects                | Unknown      | Decompress leaves and identify standard audio signatures             |
| `map0000.dat`–`map0007.dat`         | Board topology/configuration | Unknown      | Compare integer fields across maps and trace the runtime consumer    |
| `mapElem0000.dat`–`mapElem0007.dat` | Board visual elements        | Unknown      | Correlate record IDs with matching map screens                       |
| `MGame*.DAT`, `MiniGame.DAT`        | Minigame configuration       | Unknown      | Classify each file independently; do not assume one shared schema    |
| `anime.dat`                         | Animation control            | Unknown      | Inventory records and correlate with displayed frames                |
| `building.dat`                      | Building definitions         | Unknown      | Correlate values with prices, levels, and board state                |
| `events.dat`                        | Event definitions            | Unknown      | Correlate records with known triggered events                        |
| `tools.DAT`                         | Gadget/tool definitions      | Unknown      | Correlate records with gadget names and effects                      |
| `Databaseaeb8.dat`                  | Stateful save/game database  | Unknown      | Diff disposable before/after saves; never test against the only copy |

## How the formats were identified

The useful clues were cumulative:

1. **Release comparison.** Hashes and file sizes showed that many assets were
   identical, so “Chinese versus Taiwanese” was not automatically a character
   mapping oracle. The two installs also made the mistaken launch easy, which
   is why runtime tests now need an unmistakable version marker.
2. **Repeated archive signature.** Searching binaries for the GameOne author
   string exposed the shared container and nested offset tables.
3. **Executable observation.** The decompression routine and imports such as
   `DrawTextA`, `CreateFontA`, and `GetTextExtentPointA` narrowed the likely text
   paths. Imports alone were treated as clues, not proof of which path each
   screen uses.
4. **Declared output sizes.** The first dword of compressed leaves consistently
   matched the reconstructed 14-bit decoder's result.
5. **Whole-corpus validation.** A candidate string grammar succeeded for all
   945 records instead of only a convenient example.
6. **Glyph atlas rendering.** `chifont.dat` divided exactly into 32-byte blocks;
   rendering them as 16×16 bitmaps produced recognizable Chinese.
7. **Sentence consistency.** Reusing a glyph ID yielded the same character in
   many sensible sentences, allowing the index map to be checked contextually.
8. **Known file signatures.** LZW-decoded `bitmaps.dat` leaves began with valid
   PCX headers and rendered as screens seen in the game.
9. **Scanline invariants.** Sprite row commands repeatedly expanded to the
   header width, supporting the signed skip/literal interpretation.
10. **Controlled runtime tests.** Removing and patching files distinguished
    resources actually loaded by the game—after correcting the wrong-version
    test.

## Known limitations and next reverse-engineering targets

- Decode the inner `Fonts.dat` record format and identify its executable
  consumer.
- Determine the exact `interface.dat` schema and map screen IDs to bitmap and
  sprite IDs.
- Inspect the minigame `.SPR` resources for the same or a related sprite format.
- Trace the runtime formatting calls that insert numbers/names into text without
  visible printf-style markers.
- Trace how individual callers choose Sprite1 hotspots versus Sprite2 top-left
  placement, and investigate the remaining `0x8005` flag combination.
- Implement a PCX writer only after its palette, padding, and RLE invariants are
  covered by the same round-trip checks as the sprite writer.
- Determine which menu/map labels are `strings.dat` records, rendered sprites,
  or pixels baked into background art.
- Record exact executable addresses/call stacks for file loading and drawing in
  a reproducible DOSBox-X debugger session.

## Safety and reproducibility

Always test modifications against a copied installation or copied disk image.
Keep the Chinese and Taiwanese mounts visibly distinct. Preserve original
archives byte-for-byte and compare rebuilt files structurally before launching
the game. `Databaseaeb8.dat` is stateful, so use a disposable save directory
for runtime experiments.
