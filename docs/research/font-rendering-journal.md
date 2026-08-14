# Doraemon Monopoly font-rendering reverse-engineering journal

Date: 2026-08-14
Target: GameOne's 1998 Windows 95/98 _Doraemon Monopoly_ executable
(`workspace/Doraemon.exe`, SHA-256
`fdf00e681671f93b09d257f77d7ce0720e7129cf6bc44ba9e0f19c2efa4fecba`)
Scope: discovery of the sysfont/chifont text renderer, its four interception
points, and the design of the `patch_vietnamese` routine in
`crates/game-patch/src/pe.rs`

This journal records the investigation chronologically and deliberately
separates facts observed in the machine code from design decisions made while
building the patcher.

## Confidence vocabulary

- **Confirmed statically**: visible directly in the executable's instructions
  or data.
- **Confirmed dynamically**: observed while the game was running.
- **Confirmed by decoding**: reproduced by the archive/font decoder across the
  relevant records.
- **Strong inference**: supported by several independent observations, but not
  directly captured at runtime.

## Why this journal exists

A Vietnamese localization cannot render its text with the stock font
subsystem. The journey that produced the `patch_vietnamese` code has four
distinct parts, and each part has a traceable chain of evidence:

1. finding the font filenames and the code that loads them;
2. reading the loader to learn the in-memory font globals;
3. mapping the renderers that consume those globals and their high-bit
   (Chinese) branches;
4. designing an executable patch that reuses the original renderers while
   adding a CC/CD Vietnamese dispatch.

## Background: the Vietnamese encoding problem

The original `sysfont.dat` holds 640 glyphs arranged as five variants of 128
proportional ASCII glyphs. The renderer selects a glyph by a byte below `0x80`.
Bytes at or above `0x80` begin a two-byte Chinese code. The original
`chifont.dat` holds 747 headerless 16×16 glyphs (`chifontBase + id*32`).

That leaves no free one-byte slot for Vietnamese letters. The solution adopted
is to steal two *unused Chinese lead bytes* - `0xCC` and `0xCD` - and encode a
Vietnamese character as two bytes:

```text
slot 0..127   ->  CC 0x00..0x7F
slot 128..255 ->  CD 0x00..0x7F
```

The game itself never emits `0xCC` or `0xCD` as a Chinese lead byte. The
`chifont.dat` decoding confirms that no used Chinese id has a first byte in
that range (**confirmed by decoding**). Every glyph id present in the shipped
`chifont.dat` has a first byte of `0x00`, `0x01`, or `0x02`.

The sysfont is expanded from 640 to 1,920 records:

```text
0..639      original five 128-slot variants
640..1919   five Vietnamese banks of 256 records each
```

A Vietnamese slot is mapped to sysfont record:

```text
640 + activeVariant * 256 + VietnameseSlot
```

To render, the executable must be taught this mapping in the four places where
it measures and draws text. Finding those four places is the subject of this
journal.

## Part 1: finding the font filenames

### The filenames live in DSEG, not in `.rdata`

A `strings` scan over the executable finds the font filenames at these raw file
offsets:

```text
c8d3c  fonts.dat
cb00a  sysfont.dat
cb022  chifont.dat
```

The section table shows why `cb00a` and `cb022` are interesting:

```text
Idx Name    Size      VMA       LMA       File off
  3 DSEG    000000e0  004d0000  004d0000  000cb000
  4 CSEG    00001bd5  004d1000  004d1000  000cc000
```

DSEG occupies file offsets `0xcb000` through `0xcb0e0` and maps to virtual
addresses `0x4d0000` through `0x4d00e0`. Therefore:

```text
file 0xcb00a -> VA 0x4d000a  = "sysfont.dat"
file 0xcb022 -> VA 0x4d0022  = "chifont.dat"
```

`fonts.dat` at `0xc8d3c` sits in `.data` (VA `0x4c8d3c`); it is opened through
the GameOne archive loader at `0x408903` with a `0x7800` size hint and is a
different mechanism. The sysfont/chifont pair is loaded by a separate, simpler
path that this journal follows. This is **confirmed statically** by the section
table and **confirmed by decoding** of `strings`.

### The loader and the custom file-open helper

Searching the disassembly for the two DSEG addresses leads to CSEG. The font
subsystem starts at `0x004D1000`, immediately after a table of globals in
DSEG:

```asm
004d100b  movl   $0x4d000a, %eax     ; "sysfont.dat"
004d1010  movl   $0x8000, %edx
004d1015  movl   $0x100, %ebx
004d101a  pushl  %ebx
004d101b  pushl  %edx
004d101c  pushl  %eax
004d101d  calll  0x4acf71            ; custom open-file helper
```

The helper at `0x4acf71` pushes `0x40` and forwards to `0x4acf88`, which calls
imported functions whose addresses resolve to:

```text
0x4b9198  KERNEL32.dll!CreateFileA
0x4b919c  KERNEL32.dll!GetFileType
0x4b91a0  KERNEL32.dll!CloseHandle
0x4b91b4  KERNEL32.dll!GetLastError
```

So `0x4acf88` is a custom file opener built on `CreateFileA`, with error
diagnostics via `GetLastError`. This is **confirmed statically**: the import
addresses were resolved through the PE import directory.

## Part 2: reading the loader for the font globals

Continuing the same loader function after the open:

```asm
004d1025  cmpl   $0xffffffff, %eax     ; CreateFileA failure?
004d1028  je     0x4d1108              ; bail out
004d1031  calll  0x4ae501              ; file length (GetFileSize-ish)
004d103a  calll  0x4ac1ac              ; allocate memory
004d1042  movl   %eax, %ebx
004d1044  movl   %ebx, 0x4d0006        ; [DSEG+0x06] = sysfont base
004d104a  movl   %ebx, 0x4d0002
004d1050  addl   $0x2, 0x4d0002        ; [DSEG+0x02] = base + 2 (offset table)
004d1057  movl   $0x0, 0x4d001a        ; [DSEG+0x1A] = active variant 0
004d1081  calll  0x4ac6ac              ; read file into the buffer
004d1092  calll  0x4d113f              ; select variant 0
004d1098  movl   $0x4d0022, %eax       ; "chifont.dat"
...
004d10cd  movl   %ebx, 0x4d001e        ; [DSEG+0x1E] = chifont base
```

The loader reads each whole file into an allocated buffer and records the
base pointers in DSEG. The resulting global map is **confirmed statically**:

| DSEG address | Meaning                                  |
| ------------ | ---------------------------------------- |
| `0x4d0002`   | sysfont offset-table pointer (`base + 2`)|
| `0x4d0006`   | base address of loaded `sysfont.dat`     |
| `0x4d000a`   | `sysfont.dat` filename                   |
| `0x4d001a`   | active sysfont variant number            |
| `0x4d001e`   | base address of loaded `chifont.dat`     |
| `0x4d0022`   | `chifont.dat` filename                   |
| `0x4d0032`   | height of the active sysfont variant     |

### The variant selector at `0x4d113f`

```asm
004d113f  movl   0x4(%esp), %eax       ; variant number
004d1145  shll   $0x7, %eax            ; * 128
004d1148  movl   0x4d0006, %ebx        ; sysfont base
004d114e  movw   (%ebx), %bx           ; glyph count (u16)
004d1151  cmpw   %bx, %ax              ; variant*128 <= count?
004d1154  jbe    0x4d115b
004d1157  xorl   %eax, %eax            ; invalid variant
004d1159  jmp    0x4d118c
004d115b  movl   0x4d0006, %ebx
004d1161  addl   $0x2, %ebx            ; +2 -> offset table
004d1164  shll   $0x2, %eax            ; variant*128*4
004d1167  addl   %ebx, %eax
004d1169  movl   %eax, 0x4d0002        ; offset-table pointer for this variant
004d116e  movl   (%eax), %eax          ; glyph-0 offset
004d1170  addl   0x4d0006, %eax        ; + base
004d1176  incl   %eax                  ; +1: skip the width byte
004d1177  movb   (%eax), %al           ; read height
004d1179  movb   $0x0, %ah
004d117b  movw   %ax, 0x4d0032         ; store active variant height
004d1181  popl   %eax
004d1182  movl   %eax, 0x4d001a        ; store active variant number
004d1187  movl   $0xffffffff, %eax
```

This is the runtime font-variant switcher. The game stores the active variant
in `0x4d001a`; the patched Vietnamese index uses exactly that global so that a
mid-game variant switch selects the correct Vietnamese bank. This is
**confirmed statically**, and the "five variants" question from the earlier
localization work is answered: the game selects among the five 128-glyph
sysfont blocks at runtime, and each block has its own height stored in
`0x4d0032`.

### Where the game actually switches variants: escape markers

The variant selector `0x4d113f` is called from only two sites, both inside a
single text-markup interpreter: `0x432ab4` and `0x43690e`. Neither call takes a
fixed number; the variant is chosen **per string, at render time**, from an
escape marker embedded in the text data. This is why the "variant switch" is
not visible as a standalone function.

The interpreter at `0x432a00` walks the string one byte at a time. A byte equal
to `0x5c` (`\`) starts an escape sequence (`0x4329e5`). The byte after the
backslash is dispatched (`0x432a0b`):

| Marker | Action                                                 |
| ------ | ------------------------------------------------------ |
| `C`    | read three characters, convert with `atoi`, use as code|
| `F`    | read a digit, convert with `atoi`, **call `0x4d113f`** to select the sysfont variant |
| `N`    | newline: reset the width accumulator and advance the line |

So a string like `\F3text` selects variant 3 before drawing `text`. The
`'F'` branch reads its digit into a stack string and converts it with
`0x4ad2cc` (the `atoi`-style helper that also handles `+`/`-`), then pushes the
result into `0x4d113f`, which stores it in `0x4d001a` as already shown. The
second call site `0x43690e` is the same escape interpreter in a sibling layout
routine.

**The shipped game data never uses the `\F` escape.** Decoding the real dialog
archive `strings.dat` (945 records) shows zero `\F` markers. The only backslash
sequences present are `\N` (222 newlines) and a handful of `\` + Chinese lead
byte (`0x80`–`0x82`), which are literal backslashes before a two-byte code. An
earlier scan of `DORAEMON.bin` (the 409 MB CD image) found 150 `\F[0-9]` byte
runs, but every readable sample was installer/config data (`VIRTUAL\F3VirtSB`
is a Sound Blaster driver name), not dialog text. So the `\F` handler and the
five sysfont variants exist in the executable, but the boot loader always
initializes variant 0 (`movl $0,0x4d001a` at `0x4d1057`, then `0x4d113f` with
argument 0) and no shipped string switches it away.

A byte-level scan for the little-endian address `1a 00 4d 00` across the whole
executable finds exactly three touch points for `0x4d001a` - the loader init
(`c7 05`, `0x4d1057`), the getter read (`a1`, `0x4d1139`), and the selector
store (`a3`, `0x4d1182`). The selector is reachable only from the two `\F`
escape sites. Therefore **nothing in the stock game triggers a variant switch**:
`0x4d001a` is written once to 0 at boot and then only changes if a string
carrying a `\F[0-4]` escape is rendered, which no shipped record does.
**Confirmed statically** (call graph) and **confirmed by decoding** (all 945
dialog records inspected).

Because the same scan finds no save/restore of `0x4d001a` around string
rendering, a `\F` escape is a **global, sticky side-effect**: once a rendered
string sets `\F4`, every subsequent string drawn anywhere in the game uses
variant 4 until the font loader re-initializes (`0x4d1000`, single caller
`0x43741c`, guarded by the `0x4cd9fc` init flag - i.e. startup-time, not
per-string or per-scene) or another `\F[0-4]` escape overrides it. The renderer
consumes `0x4d001a` at draw time, so the switch applies to the whole game
thereafter, not just the string that set it. A translator emitting `\F4` must
emit `\F0` to restore the default.

Consequence for the Vietnamese patch: in the stock game every rendered
character uses variant 0, so `0x4d001a` reads 0 in practice. The patch reads
the same global, so a future translated string that *does* carry a `\F` escape
selects the matching Vietnamese bank automatically - but the stock text never
does.

### The sysfont glyph record layout

From the selector and from decoding `sysfont.dat`:

```text
u16  glyph count          at 0x0000
u32  offsets[count]       at 0x0002
per glyph at [base+off]:  u8 width, u8 height, then width*height pixel bytes
```

Pixel byte `0x00` is ink, `0xFF` is transparent. The shipped font uses
proportional widths: glyph `'A'` is 9×16, glyph `0` is 11×16. This is
**confirmed by decoding** of `sysfont.dat`.

## Part 3: the four text renderers and their Chinese branches

The renderers live in CSEG immediately after the loader. Only **four** of the
text functions are patched - the ones that measure or draw characters. Each has
exactly one Chinese branch, and the patch replaces that branch. The other text
helpers (`count_bytes`, the variant getter/selector) are *not* patched.

### The four renderers at a glance

| Renderer        | VA       | Role                                    | Called from `.text` |
| --------------- | -------- | --------------------------------------- | ------------------- |
| `measure_string`| `0x4d118e`| sum the pixel widths of a whole string  | `0x432ae0`, `0x43694c`, `0x48ccf1` |
| `character_width`| `0x4d120c`| width of a single character             | (no direct `call`; small helper) |
| `single_render` | `0x4d123c`| draw one character into a bitmap        | `0x432c88` |
| `string_render` | `0x4d1364`| draw a whole string into a bitmap       | `0x432d63` |

Two helpers sit between them but are **not** patched:

| Helper         | VA       | Role                                             |
| -------------- | -------- | ------------------------------------------------ |
| `count_bytes`  | `0x4d11e2`| count the number of encoded bytes (1 per ASCII, 2 per Chinese) - no glyph work |
| variant getter / selector | `0x4d1139` / `0x4d113f` | read / change `0x4d001a` (see Part 2) |

### The shared byte loop

All four renderers walk the string with the same byte loop: `lodsb`, stop at
NUL, then test bit 7 of the byte. A clear bit means ASCII (single-byte), a set
bit means the two-byte Chinese path:

```asm
ac                    lodsb            ; al = *esi++
0a c0                 or al, al
74 xx                 je done
66 0f ba e0 07        bt $0x7, ax      ; high bit set?
72 xx                 jb chinese       ; yes -> two-byte Chinese
...                   ; ASCII: glyph = [base+2 + char*4] offset, then width
```

The ASCII path resolves a glyph offset through `[0x4d0002]`, adds `[0x4d0006]`,
and reads the width byte. The Chinese path handles the two-byte code.

`measure_string` and `string_render` loop over the whole string; the two
single-character helpers (`character_width`, `single_render`) run the same
logic once for the first byte only.

### Rendering a string, byte by byte (the normal path)

Take `string_render` (`0x4d1364`) rendering the two characters `A` (ASCII
`0x41`) then a Chinese character `中` (two bytes `0xD6 0xD0`). Each iteration
of its byte loop reads one character and dispatches on bit 7:

| iteration | byte(s)  | bit 7 | what the renderer does                          |
| --------- | -------- | ----- | ----------------------------------------------- |
| 1         | `0x41`   | 0     | glyph offset = `[base+2 + 'A'*4]`; add sysfont base; read width (9 px); draw the 9×16 glyph |
| 2         | `0xD6`   | 1     | `jb` to the Chinese branch: `lodsb` the second byte (`0xD0`), `and $0x7fff` → id `0x56D0`, `*32`, add chifont base; draw the fixed 16×16 glyph |
| 3         | `0x00`   | -     | NUL → stop                                       |

There are exactly two ways into a glyph: the ASCII path (proportional sysfont
glyph) and the Chinese path (fixed 16×16 chifont glyph). **The patch adds a
third: the Vietnamese path from the expanded sysfont.** It never touches the
ASCII path, and it re-injects every non-Vietnamese high-bit byte back into the
original Chinese path.

### Measure string (`0x4d118e`)

Walks a whole string, accumulating widths in `edx` and returning the total
pixel width. The Chinese branch is at `CSEG+0x01d0` (VA `0x4d11d0`):

```asm
004d11d0  ac              lodsb            ; consume second byte
004d11d1  83 c2 10        add $0x10, %edx  ; every Chinese char is 16px
004d11d4  eb d2           jmp 0x4d11a8     ; back to the loop
```

The five original bytes at `0x4d11d0` are the machine-code signature the
patcher verifies:

```text
ac 83 c2 10 eb      ; lodsb; add edx,0x10; jmp
```

### Measure one character (`0x4d120c`)

Returns the width of a single character in `eax` (the Chinese case returns a
flat 16). The Chinese branch is at `CSEG+0x0235` (VA `0x4d1235`):

```asm
004d1235  b8 10 00 00 00   mov $0x10, %eax  ; Chinese width = 16
004d123a  5e               pop %esi
004d123b  c3               ret
```

Signature bytes:

```text
b8 10 00 00 00      ; mov eax,0x10
```

### Build one character bitmap (`0x4d123c`)

Draws a single character into a bitmap. For ASCII it reads the proportional
glyph from sysfont; for Chinese it reads the fixed 16×16 glyph from chifont.
The Chinese branch is at `CSEG+0x02e1` (VA `0x4d12e1`):

```asm
004d12e1  8a e0            mov %al, %ah        ; keep first byte
004d12e3  ac               lodsb               ; second byte
004d12e4  66 25 ff 7f      and $0x7fff, %ax    ; clear lead bit
004d12e8  0f b7 c0         movzwl %ax, %eax
004d12eb  c1 e0 05         shl $0x5, %eax      ; * 32
004d12ee  03 05 1e 00 4d 00 add 0x4d001e, %eax ; + chifont base
004d12f4  8b f0            mov %eax, %esi
```

The Chinese id is `((first & 0x7f) << 8) | second`, and the glyph is a 16×16
bitmap at `chifontBase + id*32`. Signature bytes:

```text
8a e0 ac 66 25      ; mov ah,al; lodsb; and ax,0x7fff
```

### Build a full-string bitmap (`0x4d1364`)

Draws a whole string into a bitmap, one glyph per row. It first calls
`measure_string` (`0x4d118e`) to size the buffer. Its Chinese branch is at
`CSEG+0x0444` (VA `0x4d1444`), identical logic plus a `push esi` before
masking:

```asm
004d1444  8a e0            mov %al, %ah
004d1446  ac               lodsb
004d1447  56               push %esi
004d1448  66 25 ff 7f      and $0x7fff, %ax
```

Signature bytes:

```text
8a e0 ac 56 66      ; mov ah,al; lodsb; push esi; and ax,0x7fff
```

These four signatures are exactly the five-byte guards that `font_layout()`
checks in `pe.rs` before patching (**confirmed statically** against the file
bytes at `0xcc1d0`, `0xcc235`, `0xcc2e1`, and `0xcc444`).

## Part 4: the executable patch

### Same string, patched - where the injection hooks in

The patch does **not** wrap a renderer. For each of the four Chinese branches
it rewrites exactly **five bytes** - the first five bytes of the branch - into
an `e9 <rel32>` near jump into the cave. Everything else in the function is
byte-for-byte identical. Let's read that literally on the `string_render`
branch at `0x4d1444`, because one detail there explains the whole design.

**Before - the five bytes being replaced** (each `#` marks one byte inside
the five-byte window):

```asm
addr       bytes        instruction          window
0x4d1444   8a e0        mov %al, %ah         #1 #2
0x4d1446   ac           lodsb                #3
0x4d1447   56           push %esi            #4
0x4d1448   66 25 ff 7f  and $0x7fff, %ax     #5 (only the `66` prefix lies inside)
0x4d144c   0f b7 c0     movzwl %ax, %eax     -- outside the window; kept as is
```

**After - the same five bytes:**

```asm
addr       bytes        instruction          window
0x4d1444   e9 .. .. .. ..  jmp cave_stub     #1 #2 #3 #4 #5
0x4d1449   25 ff 7f     (orphaned tail of the `and`)  -- dead, never executed
0x4d144c   0f b7 c0     movzwl %ax, %eax     -- outside the window; kept as is
```

Notice what happened to the `and $0x7fff, %ax` instruction. It is four bytes
long (`66 25 ff 7f`), so only its **first byte** (`66` at `0x4d1448`) lies
inside the window. The jump wipes that byte and leaves a broken `25 ff 7f`
stranded in the file. It is dead code - execution never reaches it, because
`0x4d1444` is now an unconditional jump.

That one asymmetry is the reason every cave stub has a fallback that
*replays* the original instructions. The original code that turned two bytes
into a chifont id was:

```asm
mov %al, %ah        ; keep the first byte in ah
lodsb               ; second byte -> al
push %esi           ; park esi
and $0x7fff, %ax    ; id = ((first & 0x7f) << 8) | second
```

Those four instructions straddle the window, so the patch erased them. For
any byte that is *not* Vietnamese - real Chinese, or any regional high-bit
code - the cave must recreate them exactly, which is precisely what the
`string_chinese` branch does (pe.rs:116-118), before jumping back to
`0x4d144c`, the first intact instruction *after the window*. From there the
original code (bitmap building, coordinate advance) continues untouched.

Now run a three-character string through the patched `string_render` - `A`,
the Chinese `中` (`0xD6 0xD0`), and a Vietnamese `à` (`CC 0x20`):

| iteration | byte(s)     | dispatch in the stub                                 | then                                                                                            |
| --------- | ----------- | ---------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| 1         | `0x41`      | - (ASCII; the stub is never reached)                 | original ASCII path, unchanged                                                                  |
| 2         | `0xD6 0xD0` | not `CC`/`CD` → `string_chinese` fallback            | stub replays `mov ah,al; lodsb; push esi; and ax,0x7fff`, then `jmp 0x4d144c` - the first intact instruction after the window; original chifont logic continues |
| 3         | `CC 0x20`   | `CC` and second byte `< 0x80` → Vietnamese           | stub computes sysfont record `640 + variant*256 + 32`, loads the glyph the way the ASCII path does, then `jmp 0x4d140b` (the row-draw loop entry); original bitmap building draws it |

Key invariants the injection relies on:

- Each stub re-enters original code at the **first intact instruction after
  its five-byte window** (`0x4d144c` here), *not* at the byte right after the
  window - because the stub itself re-emits any instruction that straddled the
  boundary. Register state (`al`, `esi`, `edx`, flags) must therefore match
  what the original branch would have produced at that point. That is why each
  stub either replays the original bytes (`string_chinese`) or reproduces
  their effect (`inc esi` in `measure_string`) before the jump back.
- The four re-entry points are `CSEG+0x01a8` (measure loop head),
  `CSEG+0x023a` (width pop/ret), `CSEG+0x0281` / `CSEG+0x02e8`
  (single-render draw / chifont continuation), and `CSEG+0x040b` /
  `CSEG+0x044c` (string-render draw / chifont continuation).

### Where the code cave fits

CSEG has raw size `0x2000` but a virtual size of only `0x1bd5`. The raw bytes
from `CSEG+0x1c00` through `CSEG+0x1fff` are therefore present on disk but not
mapped. Raising the section's virtual size to `0x2000` maps exactly 1,024
bytes of free space - the cave - at VA `0x4d2c00`. The patcher:

1. verifies the four five-byte signatures;
2. writes the dispatch stubs into the cave at `CSEG+0x1c00`;
3. rewrites the four branch entries as five-byte near jumps into the cave;
4. raises the CSEG virtual size to `0x2000`;
5. adds the section characteristic.

### The dispatch stubs

Each cave stub re-uses the byte loop structure of the original renderer. On
entry, `al` holds the first byte and `esi` points at the second byte. The
shared prefix check is:

```asm
3c cc            cmp $0xcc, %al
0f 85 ..         jne check_cd
e9 ..            jmp  second_ok
check_cd:
3c cd            cmp $0xcd, %al
0f 85 ..         jne chinese_fallback
second_ok:
f6 06 80         testb $0x80, (%esi)
0f 85 ..         jne chinese_fallback
```

A `CC`/`CD` lead followed by a second byte below `0x80` is Vietnamese;
everything else falls through to the original Chinese path byte-for-byte. This
is what keeps the pre-existing game text (and any regional build's own high-bit
codes) rendering identically.

### The Vietnamese index

```asm
0f b6 c0          movzbl %al, %eax
2d cc 00 00 00    sub $0xcc, %eax          ; 0 for CC, 1 for CD
c1 e0 07          shl $7, %eax             ; *128
0f b6 0e          movzbl (%esi), %ecx      ; second byte
01 c8             add %ecx, %eax           ; slot = (lead-0xcc)*128 + second
8b 0d ..          mov [0x4d001a], %ecx     ; active variant
83 f9 04          cmp $4, %ecx
76 02             jbe +2
31 c9             xor %ecx, %ecx           ; clamp invalid variant to 0
c1 e1 08          shl $8, %ecx             ; variant*256
01 c8             add %ecx, %eax
05 80 02 00 00    add $640, %eax           ; + sysfont base slot
```

This yields record `640 + variant*256 + slot`, exactly the layout of the
expanded `sysfont.dat` produced by `sysfont.rs`. The result is a sysfont glyph
index.

### Reusing the original glyph machinery

After computing the index, each stub loads the glyph record the same way the
original ASCII path does:

```asm
8b 0d ..          mov [0x4d0006], %ecx     ; sysfont base
8b 44 81 02       mov 0x2(%ecx,%eax,4), %eax  ; offset-table entry
01 c8             add %ecx, %eax           ; glyph record address
0f b6 00          movzbl (%eax), %eax      ; width byte
```

- **measure_string** adds that real width to `edx`, advances `esi` past the
  second byte, and jumps back to the loop head at `0x4d11a8`.
- **character_width** leaves the width in `eax` and returns through the
  original `pop esi; ret` at `0x4d123a` (restoring the `ecx` it borrowed).
- **single_render** and **string_render** put the glyph record address into
  `esi` and jump into the middle of the original bitmap-building code
  (`0x4d1281` and `0x4d140b` respectively), so the Vietnamese glyphs are
  rasterized by the original renderer with no duplicated drawing logic.

Because the original machine code at each of the four return points is
re-entered at the exact instruction that follows the overwritten five bytes,
the stack and register conventions of every caller are preserved. This is the
key reason the patch is five-byte jumps plus a cave rather than a rewrite of
the renderers.

## How this maps to `patch_vietnamese` in `pe.rs`

| Concept in the executable | Constant / site in `pe.rs`                         |
| ------------------------- | -------------------------------------------------- |
| CSEG virtual base `0x4d1000` | `CSEG_VA` discovered from the section table   |
| CSEG raw offset `0xcc000` | discovered from the section table                |
| four Chinese branch sites | `0x01d0`, `0x0235`, `0x02e1`, `0x0444` relative to CSEG |
| four five-byte signatures  | the `font_layout()` guard bytes                  |
| cave base `CSEG+0x1c00`   | `cave_va = cseg_va + 0x1c00`                     |
| cave capacity             | `0x400` bytes, checked by `cave.len() > 0x400`   |
| active variant `0x4d001a` | `dseg_va + 0x1a` in `vietnamese_index`            |
| sysfont base `0x4d0006`   | `dseg_va + 0x06` in every stub                    |
| record mapping `640 + variant*256 + slot` | `add eax,640` plus the variant shift |
| five-byte near jumps       | `patch_cseg_jump`                                 |
| section virtual size bump  | `output[section+8..12] = 0x2000`                 |

The reason the patcher does not hard-code raw file offsets is portability: the
two prior sections in the repo (`.port` etc.) and future builds are discovered
via the PE section table, and every assumption is defended by the four
signature checks, so an unrecognized executable fails closed instead of being
corrupted.

## Open items

- A runtime DOSBox-X pass over the patched CSEG is still the final proof that
  all four stubs return to the correct instructions under real layout and
  ownership conditions (the legacy notes in
  [`legacy/EXECUTABLE_FONT_RESEARCH.md`](legacy/EXECUTABLE_FONT_RESEARCH.md)
  list the same verification steps).
- The `\F` escape handler exists in the interpreter (`0x432a00`/`0x43690e`
  calling `0x4d113f`), but a whole-executable byte scan finds only three
  touches of `0x4d001a` (loader init, getter read, selector store) and the
  stock `strings.dat` carries no `\F` markers - so `0x4d001a` stays 0 and
  **nothing in the stock game changes the variant**. This is closed for the
  dialog path; the map/event/UI archives decode with a different layout and
  were not individually scanned, but the call graph already limits any switch
  to the two `\F` escape sites.

## Condensed discovery chain

```text
strings scan: sysfont.dat @ cb00a, chifont.dat @ cb022
  -> section table: cb000 is DSEG (VA 0x4d0000)
  -> filenames at 0x4d000a and 0x4d0022

search references to 0x4d000a / 0x4d0022 in CSEG
  -> loader at 0x4d1000 uses custom open helper 0x4acf71
  -> helper is CreateFileA + GetFileType + CloseHandle + GetLastError

read the loader body
  -> [0x4d0006] sysfont base, [0x4d0002] offset table,
     [0x4d001a] active variant, [0x4d001e] chifont base,
     [0x4d0032] active height

disassemble CSEG after the loader
  -> four renderers: measure_string 0x4d118e,
     character_width 0x4d120c, single_render 0x4d123c,
     string_render 0x4d1364
  -> count_bytes 0x4d11e2 is a helper, not patched

find the high-bit (Chinese) branch in each renderer
  -> 0x4d11d0, 0x4d1235, 0x4d12e1, 0x4d1444
  -> capture the five-byte signatures

find where variants are switched
  -> callers of the selector 0x4d113f: 0x432ab4, 0x43690e
  -> both are the \F escape branch of the text interpreter
     at 0x432a00; \N newline and \C code markers too
  -> but stock strings.dat (945 records) has zero \F markers:
     the game boots to variant 0 and no shipped dialog switches it

design the CC/CD dispatch
  -> CC/CD lead + second byte < 0x80 = Vietnamese slot
  -> slot -> 640 + variant*256 + slot via [0x4d001a]
  -> reuse original width/bitmap machinery

place the cave at CSEG+0x1c00, patch four near jumps
  -> raise CSEG virtual size to 0x2000
```
