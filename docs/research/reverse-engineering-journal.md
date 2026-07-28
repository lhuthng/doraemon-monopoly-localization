# Doraemon Monopoly resource-loader reverse-engineering journal

Date: 2026-07-14  
Target: GameOne's 1998 Windows 95/98 _Doraemon Monopoly_ executable  
Scope: discovery of `strings.dat` loading, string-group objects, gadget-record
lookup, record-cache behavior, and the proposed cache-size patch

This journal records the investigation chronologically. It deliberately
separates facts observed in files or machine code from interpretations that
still require a final runtime breakpoint confirmation.

## Confidence vocabulary

- **Confirmed statically**: visible directly in the executable's instructions
  or data.
- **Confirmed dynamically**: observed while the game was running in the
  DOSBox-X debugger.
- **Confirmed by format decoding**: reproduced by the archive decoder across
  the relevant records.
- **Strong inference**: supported by several independent observations, but a
  specific runtime branch has not yet been captured.

## Starting point: decoded `strings.dat`

The GameOne archive decoder showed that `strings.dat` is a recursive archive.
Its root contains nine child containers, indexed `0` through `8`. Those
containers contain the following numbers of leaf records:

| Outer group | Leaf count |
| ----------: | ---------: |
|       `000` |         36 |
|       `001` |         42 |
|       `002` |         51 |
|       `003` |        136 |
|       `004` |        136 |
|       `005` |        136 |
|       `006` |        136 |
|       `007` |        136 |
|       `008` |        136 |

This totals 945 leaf records. The decoded content established that group
`001` contains the gadget/tool names and descriptions. Its final record is
therefore identified as `001/041`.

At this stage, the nine-group structure and the text contents were known from
the resource file. What was not yet known was how the executable represented
those groups in memory or which game code requested a gadget string.

## Finding the executable code that opens `strings.dat`

### Locate the literal filename

The executable data section contains a cluster of resource filenames:

```text
004C8D14  "Tools.dat"
004C8D20  "Building.dat"
004C8D30  "Events.dat"
004C8D3C  "fonts.dat"
004C8D48  "strings.dat"
004C8D54  "voice.dat"
```

`"strings.dat"` therefore resides at virtual address `004C8D48` in this
executable build. Searching the disassembly for a reference to that address
led to the initialization code at `00456556`:

```asm
00456556  push 004C8D48        ; "strings.dat"
0045655B  call 0048243B
00456560  add  esp,4
00456563  mov  [ebp-260],eax
```

The same function at `0048243B` is called with other known resource
filenames. Its return value is subsequently passed to functions that query
archive children. This identifies it as the GameOne file/archive opener.

Equivalent pseudocode:

```c
rootArchive = openGameOneArchive("strings.dat");
```

This is **confirmed statically**.

## Finding the hardcoded nine-group loop

Immediately after opening `strings.dat`, the executable initializes a loop
counter and compares it with the literal value nine:

```asm
00456569  mov  [ebp-240],0      ; index = 0
...
00456584  cmp  [ebp-240],9
0045658B  jge  00456638         ; leave loop when index >= 9
```

The loop increment is:

```asm
00456575  mov  ecx,[ebp-240]
0045657B  add  ecx,1
0045657E  mov  [ebp-240],ecx
```

This independently confirms that the game expects exactly nine top-level
children in `strings.dat`. The agreement with the decoded archive is not an
assumption: the file decoder and executable analysis reached the same number
through different paths.

## Retrieving each child archive

Inside the loop, the executable passes the root archive and current index to
`004826F1`:

```asm
00456591  mov  edx,[ebp-240]    ; current group index
00456597  push edx
00456598  mov  eax,[ebp-260]    ; root strings.dat archive
0045659E  push eax
0045659F  call 004826F1
004565A4  add  esp,8
004565A7  mov  ecx,[ebp-240]
004565AD  mov  [ebp+ecx*4-28],eax
```

The returned values are kept in a temporary array indexed by the loop
counter. In pseudocode:

```c
childArchives[index] = getArchiveChild(rootArchive, index);
```

Because the archive format uses ordered child offsets, child index `0`
corresponds to group `000`, index `1` to group `001`, and so on.

## Constructing one loader object per group

For each child archive, the game allocates `0x44` bytes:

```asm
004565B1  push 44
004565B3  call 004AC674         ; allocate 0x44 bytes
```

If allocation succeeds, it invokes the constructor at `00408310`:

```asm
004565CA  mov  ecx,[allocatedObject]
004565D0  call 00408310
```

The resulting loader pointer is then placed in a global array:

```asm
004565F3  mov  eax,[ebp-240]                 ; index
004565F9  mov  ecx,[newLoader]
004565FF  mov  [004CC9E8 + eax*4],ecx
```

Equivalent pseudocode:

```c
groupLoaders[index] = new GameOneArchiveLoader();
```

Since this is a 32-bit executable, every pointer occupies four bytes. The
array mapping is therefore:

| Group index | Loader-pointer slot |
| ----------: | ------------------: |
|       `000` |          `004CC9E8` |
|       `001` |          `004CC9EC` |
|       `002` |          `004CC9F0` |
|       `003` |          `004CC9F4` |
|       `004` |          `004CC9F8` |
|       `005` |          `004CC9FC` |
|       `006` |          `004CCA00` |
|       `007` |          `004CCA04` |
|       `008` |          `004CCA08` |

This mapping follows directly from `004CC9E8 + index * 4` and is **confirmed
statically**.

## Finding the `0x400` cache-size argument

Still inside the same nine-iteration loop, the executable initializes each
loader:

```asm
00456606  push 00010000
0045660B  push 00000400
00456610  mov  edx,[ebp-240]
00456616  mov  eax,[ebp+edx*4-28]          ; child archive
0045661A  push eax
0045661B  mov  ecx,[ebp-240]
00456621  mov  ecx,[004CC9E8+ecx*4]        ; loader object
00456628  call 00408735
```

Equivalent pseudocode:

```c
initializeArchiveLoader(
    groupLoaders[index],
    childArchives[index],
    0x400,
    0x10000
);
```

Disassembling `00408735` showed what the arguments mean. The `0x400`
argument is compared with the largest individual leaf requirement, passed to
the allocator, and stored in the loader's capacity/free-space fields:

```asm
; Allocate the cache buffer using the selected capacity.
00408867  mov  ecx,[ebp+0C]
0040886A  push ecx
0040886B  call 0048CD91
00408873  mov  [loader+18],eax

; Record cache capacity and initially available space.
004088AF  mov  [loader+0C],cacheSize
004088B8  mov  [loader+10],cacheSize
```

Therefore:

```text
0x400 hexadecimal = 1,024 decimal bytes
```

The exact 1 KiB cache value is **confirmed statically**. It was not estimated
from file sizes or screenshots.

The `0x10000` argument is a separate flags/configuration value. It is not the
cache capacity.

## Connecting group `001` to the gadget accessor

With the global array identified, the next step was to search the executable
for references to the group-`001` slot, `004CC9EC`.

The decisive reference appears in the small function at `0048F676`:

```asm
0048F676  push ebp
0048F677  mov  ebp,esp
0048F679  sub  esp,8
0048F67C  mov  [ebp-8],ecx
0048F67F  mov  eax,[ebp-8]
0048F682  mov  ecx,[eax+0C]       ; record index stored in gadget object
0048F685  push ecx                ; loadRecord(recordIndex)
0048F686  mov  ecx,[004CC9EC]     ; group 001 loader
0048F68C  call 0040896F
0048F691  mov  [ebp-4],eax
0048F694  mov  eax,[ebp-4]
0048F697  mov  esp,ebp
0048F699  pop  ebp
0048F69A  ret
```

Equivalent pseudocode:

```c
char *getGadgetText(Gadget *gadget) {
    int recordIndex = gadget->field_0C;
    return groupLoaders[1]->loadRecord(recordIndex);
}
```

This is the origin of the debugger breakpoint at `0048F68C`. It is not a
generic renderer breakpoint; it is the precise call through which a gadget
object requests its group-`001` string record.

## The adjacent title-extraction function

The function immediately following the accessor, starting at `0048F69B`,
strengthens the identification. It obtains the gadget string, scans bytes,
handles two-byte Chinese glyph references, and replaces a backslash with a
NUL byte:

```asm
0048F6BC  add  eax,[currentOffset]
0048F6BF  movsx ecx,byte [eax]
0048F6C2  test ecx,ecx
0048F6C4  je   0048F705             ; stop at NUL
...
0048F6CF  and  eax,80               ; high-bit Chinese lead byte?
0048F6D6  je   0048F6E3
0048F6D8  add  currentOffset,1       ; skip second glyph byte
...
0048F6EC  cmp  eax,5C               ; ASCII backslash
0048F6EF  jne  0048F6FA
0048F6F7  mov  byte [ecx],00         ; split string at backslash
```

For a record such as:

```text
Homing Missile\N[instant]: Place ...
```

the mutation produces:

```text
Homing Missile\0N[instant]: Place ...
```

The first C-style string is now only the gadget title. This behavior matches
the game's use of the first line as an item name.

## Dynamic proof: record `001/041`

A DOSBox-X execution breakpoint was placed at:

```text
BP 0137:0048F68C
```

At this point the call has not yet executed. The immediately preceding
`push ecx` placed the record index on the stack. For the final gadget, the
first four bytes at `SS:ESP` were:

```text
29 00 00 00
```

As a little-endian 32-bit integer:

```text
0x00000029 = 41 decimal
```

This dynamically confirmed that the gadget accessor was requesting record
`001/041`.

The post-call breakpoint was:

```text
BP 0137:0048F691
```

At this address, `EAX` contains the return value from `0040896F`.

### Working `3839.dat`

The working file returned a nonzero pointer:

```text
EAX = 00DAB020
```

Displaying `DS:EAX` showed the decoded bytes:

```text
Homing Missile\N[instant]: Place on your land to blow an opponent to a Bad Luck Zone.
```

This is **confirmed dynamically**.

### Failing exported file

The failing file returned:

```text
EAX = 00000000
```

Thus the immediate failure was not incorrect glyph rendering. The caller did
not receive a valid record pointer at all. This is also **confirmed
dynamically**.

## Disassembling the record loader at `0040896F`

The record loader performs these major operations:

1. Confirm that the GameOne subsystem is initialized.
2. Confirm that the archive object is open.
3. Validate `0 <= recordIndex < recordCount`.
4. Return an existing cached pointer if the record is already resident.
5. Obtain the record's decoded size.
6. Check available cache space.
7. Attempt to evict unused cached records when space is insufficient.
8. Return `NULL` with error `0x8004` if enough space still cannot be found.
9. Otherwise allocate cache space, read/decompress the record, and return its
   pointer.

Simplified pseudocode:

```c
void *loadRecord(ArchiveLoader *loader, int recordIndex) {
    if (!gameOneSubsystem)
        return NULL;

    if (!loader->archive)
        return NULL;

    if (recordIndex < 0 || recordIndex >= loader->recordCount)
        return NULL;

    if (loader->records[recordIndex].cachedPointer) {
        loader->activeReferenceCount++;
        return loader->records[recordIndex].cachedPointer;
    }

    unsigned required = getDecodedRecordSize(recordIndex);
    unsigned available = getAvailableCacheSpace(loader);

    while (available < required) {
        if (!evictOneUnusedRecord(loader))
            break;
        available = getAvailableCacheSpace(loader);
    }

    if (available < required) {
        loader->error = 0x8004;
        return NULL;
    }

    void *result = allocateFromLoaderCache(loader, required);
    readAndDecompressRecord(result, recordIndex, required);
    return result;
}
```

The decisive final comparison is:

```asm
00408BA2  mov  edx,[ebp-18]     ; available cache space
00408BA5  cmp  edx,[ebp-10]     ; required decoded size
00408BA8  jb   00408C83          ; available < required
```

The failure branch sets error `0x8004` and returns zero:

```asm
00408C83  mov  edx,[ebp-1C]
00408C86  mov  dword [edx+24],00008004
...
00408C99  xor  eax,eax
00408C9B  jmp  00408CCA
```

Other early validation errors also return zero, so one final runtime capture
at `00408C83` is required to prove that the failing English file takes this
specific cache-exhaustion branch.

## Why records 38 and 39 affect record 41

The decoded sizes observed in the working and fuller English files were:

|                  Record | Working `3839.dat` | Fuller English file |
| ----------------------: | -----------------: | ------------------: |
|               `001/038` |           10 bytes |           119 bytes |
|               `001/039` |           10 bytes |            83 bytes |
|               `001/040` |           89 bytes |           111 bytes |
|               `001/041` |           86 bytes |           103 bytes |
| **Total of these four** |      **195 bytes** |       **416 bytes** |

Making record 38 short saves 109 decoded bytes. Making record 39 short saves
73 decoded bytes. Making both short saves 182 bytes before considering the
other differences.

The loader cache is shared by the currently resident records of one group;
it is not a separate 1 KiB allocation for each record. Each resident record
also incurs allocator metadata/alignment overhead. Consequently, this is a
threshold problem:

```text
other resident group-001 records
+ record 38 allocation
+ record 39 allocation
+ record 40 allocation
+ record 41 allocation
+ allocator overhead/alignment
--------------------------------
must fit in the group's cache
```

The observed behavior is consistent with that threshold:

- Shortening only record 38 does not free enough space.
- Shortening only record 39 does not free enough space.
- Shortening both crosses the threshold, and record 41 receives a valid
  pointer and displays correctly.
- Filling both consumes enough of the remaining cache that record 41 receives
  a null pointer.

Which failure becomes visible depends on which allocation first crosses the
threshold and whether its caller checks for `NULL`. One caller may show an
empty description or garbage, while another may dereference `NULL` and crash.

This explanation is a **strong inference** supported by the disassembly,
decoded sizes, and runtime return values. The breakpoint below is the final
confirmation step.

## Final cache-exhaustion confirmation breakpoint

Use the failing `strings.dat`, restart the game to begin with a clean resource
state, and set:

```text
BP 0137:00408C83
```

Then open the bag or shop and exercise the gadget records. If execution stops
at `00408C83`, the loader reached its explicit insufficient-cache branch.

At that breakpoint, inspect the loader's local variables with:

```text
D SS:EBP-18
```

The relevant stack locals are:

| Stack local | Meaning                                         |
| ----------- | ----------------------------------------------- |
| `[EBP-18]`  | available cache space from the free-space query |
| `[EBP-10]`  | required decoded size of the requested record   |

Both are little-endian 32-bit values. The branch is reached only when the
available value is smaller than the required value after eviction attempts.

## Proposed executable cache patch

The initialization instruction is:

```asm
0045660B  68 00 04 00 00    push 00000400
```

Because `push imm32` is five bytes long regardless of the immediate value,
the capacity can be increased without moving any following code:

```asm
0045660B  68 00 40 00 00    push 00004000
```

This changes the per-group cache from:

```text
0x0400 =  1,024 bytes
0x4000 = 16,384 bytes
```

For this PE layout, the instruction's virtual address and file offset are:

| Location type                  |      Value |
| ------------------------------ | ---------: |
| Runtime virtual address        | `0045660B` |
| RVA from image base `00400000` | `0005660B` |
| Raw file offset                | `0005660B` |

The original bytes must always be verified before patching:

```text
File offset 0x5660B
Expected: 68 00 04 00 00
Patch to: 68 00 40 00 00
```

The initialization is inside the nine-group loop, so the change applies to
all nine string-group loader objects. The maximum reservation changes from
approximately `9 × 1 KiB = 9 KiB` to `9 × 16 KiB = 144 KiB`, which is modest
for the target environment.

A 1 MiB value is unnecessary. Because the same instruction initializes all
nine groups, a 1 MiB setting could reserve approximately 9 MiB and introduce
avoidably greater pressure on the game's heap.

No executable patch should be treated as validated until the following tests
pass:

1. Confirm the unpatched failing file reaches `00408C83`.
2. Back up the executable.
3. Verify the five original bytes at raw offset `0x5660B`.
4. Patch only the four-byte immediate from `0x00000400` to `0x00004000`.
5. Start the game with the formerly failing `strings.dat`.
6. Open the bag and every shop page.
7. Verify gadget records 38 through 41.
8. Verify the gained-item message path.
9. Exercise dialogue groups `003` through `008` to catch regressions.

## Condensed discovery chain

```text
Decode strings.dat
  -> observe nine ordered outer groups
  -> identify group 001 as 42 gadget records

Find "strings.dat" literal at 004C8D48
  -> find its reference at 00456556
  -> identify the archive-opening call
  -> observe hardcoded loop index < 9
  -> observe child retrieval by index
  -> observe loader storage at 004CC9E8 + index*4

Calculate group 001 loader slot = 004CC9EC
  -> search executable references to 004CC9EC
  -> find gadget accessor at 0048F676
  -> identify record-load call at 0048F68C

Break at 0048F68C
  -> stack argument 0x29 proves record 41

Break after call at 0048F691
  -> working file returns pointer to Homing Missile text
  -> failing file returns EAX = 0

Disassemble loader 0040896F
  -> identify validation, cache lookup, eviction, allocation
  -> identify insufficient-cache return at 00408C83

Trace loader initialization 00408735
  -> prove 0x400 is cache capacity
  -> identify patch site 0045660B

Compare decoded record sizes
  -> explain why shortening both records 38 and 39 crosses the threshold
  -> propose 0x4000 cache capacity, pending final runtime confirmation
```

## Important distinction: file size versus runtime cache use

The total compressed size of `strings.dat` does not determine whether a
record fits in the cache. Each leaf is individually compressed, but the cache
stores decoded bytes. The relevant variables are:

- decoded size of the records currently resident in one group;
- allocator metadata and alignment;
- which cached records are still referenced and cannot be evicted;
- record access order;
- the fixed per-group capacity.

This explains how a working `strings.dat` can be larger as a file while a
smaller file fails at runtime. Compressed archive size and live decoded-cache
pressure are different quantities.

## Current conclusion

The executable's architecture is now substantially mapped:

- `strings.dat` is opened from a literal filename reference.
- The executable independently expects nine top-level string groups.
- Each group receives its own loader object.
- The loader pointers are stored in a global array beginning at `004CC9E8`.
- Group `001` is stored at `004CC9EC`.
- Gadget objects request group-`001` records through `0048F68C`.
- A working record request returns a pointer to decoded text.
- The failing record-41 request returns `NULL`.
- Each group loader is initialized with a `0x400`-byte cache.
- The loader contains an explicit insufficient-cache branch that returns
  `NULL` with error `0x8004`.
- The record-length experiments closely match shared-cache threshold
  behavior.

The one remaining proof step is to capture the failing build at
`00408C83`. Once that happens, increasing the initialization immediate from
`0x400` to `0x4000` becomes a directly justified executable patch rather than
only a strong hypothesis.
