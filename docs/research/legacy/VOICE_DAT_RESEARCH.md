# Voice.dat research notes

`Voice.dat` is a nested GameOne archive. Its leaf path is
`character/bank/slot`, with six character roots in this order:

1. Doraemon
2. Nobita
3. Dorami
4. Shizuka
5. Suneo
6. Gian

Two archive topologies are currently known:

| Release                | Slots in each character root | Total leaf records |
| ---------------------- | ---------------------------- | -----------------: |
| Version 1.26 Cantonese | `84 + 64 + 42`               |              1,140 |
| Version 1.18 Chinese   | `89 + 64 + 42 + 37`          |              1,392 |

## How the mapping was found

The archive was first traversed as a GameOne tree rather than treated as one
flat audio file. Every leaf consistently has three coordinates. The root has
six children, and listening tests identify them as the six playable
characters. This produces the path `character/bank/slot`.

The useful mapping clues came from comparing independent structures:

- String groups `003` through `008` are the six character dialogue groups.
  Voice bank `0` has the same slot order, and playback matches the dialogue at
  the same record number. Therefore string `003/017` maps to voice
  `000/000/017`, `004/017` maps to `001/000/017`, and so on.
- String group `001` contains exactly 42 gadget records. Voice bank `2` also
  contains exactly 42 slots under every character, and playback identifies
  gadget names. This gives a direct `bank 2 slot N -> string 001/N` mapping.
- Voice bank `1` has 64 slots. Executable call sites and matching record counts
  show that slots `0–27` correspond to global string records
  `000/008–000/035`. The character root chooses the speaker. A physical empty
  marker means that release has no recording for that character and event.
- The attached character-selection graphic contains 36 symbols in this exact
  order: `A–Z`, then `0–9`. Bank `1` slots `28–63` also contain exactly 36
  positions. Known playback anchors, including slot `52 = Y`, `54 = 0`, and
  `55–63 = 1–9`, confirm that this is the same sequence.
- Version 1.18 Chinese adds bank `3` with 37 slots per character. Its contents
  form a sparse continuation of character-dialogue records after string slot
  `88`, but the stock executable does not load this bank.

The alphabet mapping is therefore:

```text
slot 28 = A    slot 36 = I    slot 45 = R
slot 46 = S    slot 48 = U    slot 50 = W
slot 52 = Y    slot 53 = Z    slot 54 = 0
slot 55 = 1    ...            slot 63 = 9
```

This also resolves one misleading listening result: if slot `52` is `Y` and
slot `54` is `0`, slot `48` must be `U`; `W` is slot `50`.

With the full archive path included, some concrete Doraemon-root examples are:

```text
000/001/028 = A
000/001/048 = U
000/001/050 = W
000/001/052 = Y
000/001/054 = 0
000/001/055 = 1
000/001/063 = 9

000/002/000 = gadget voice for string 001/000
000/002/041 = gadget voice for string 001/041
```

## Current semantic map

| Voice bank         | Meaning                                  | Link                                                        |
| ------------------ | ---------------------------------------- | ----------------------------------------------------------- |
| `0`                | Character dialogue                       | Known direct subset uses the same slot in groups `003–008`  |
| `1`, slots `0–27`  | Character rendering of global text       | Slot `N` corresponds to global string `000/(N + 8)`         |
| `1`, slots `28–63` | Spoken `A–Z`, `0–9`                      | Symbol is `ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789[slot - 28]` |
| `2`                | Gadget names                             | Same slot in gadget string group `001`                      |
| `3`                | Archived version 1.18 dialogue extension | Sparse relation to string slots `89–130`; not loaded by EXE |

### Version 1.18 bank-3 dialogue map

Bank `3` continues character dialogue after bank `0` reaches slot `88`. The
mapping deliberately skips string records whose mapped voice slot is empty:

| Character string slot | Voice bank/slot |
| --------------------- | --------------- |
| `89–100`              | `3/0–11`        |
| `101`                 | no voice        |
| `102`                 | `3/12`          |
| `103`                 | no voice        |
| `104–124`             | `3/13–33`       |
| `125–126`             | no voice        |
| `127`                 | `3/34`          |
| `128`                 | `3/35`          |
| `129`                 | no voice        |
| `130`                 | `3/36`          |

The character root is unchanged. For example, string `003/102` corresponds to
voice `000/003/012`, while string `008/130` corresponds to `005/003/036`.
Translation Studio can show these archived recordings beside their dialogue
records and does not repeat bank `3` in the additional-audio section. Replacing
one does not make the stock version 1.18 executable play it; that would require
a separate executable patch that expands its voice-bank table.

Version 1.26 Cantonese has no equivalent fourth archive bank: each character
root genuinely ends after banks `0–2`. Its executable opens `Voice.dat`, builds
a six-character by three-bank playback table, and its character-voice helper
reads from that table. Version 1.18 performs the same three-bank initialization
despite physically carrying a fourth bank. The extra 222 leaves are therefore
orphan archive content in the unmodified 1.18 game, not records reached by a
hidden late-dialogue loader. `Sfx.dat` is loaded through a separate effects
path.

## Shared strings in group 000

Global strings are not covered by the simple character-group mapping. For
example, `000/004` and `000/005` form one dynamic sentence:

```text
啊！只要 + [inserted number] + 個豆沙包，很便宜呢！把它買下來吧！
```

This pair is constructed display text. Neither `000/004` nor `000/005` owns a
voice record, and Translation Studio must not attach character audio controls
to either half. The same rule applies to `000/006 + 000/007`.

Static analysis of the version 1.18 Chinese executable resolves two of these event links. The
voice playback helper at `0x004A7F20` receives `(character, bank, slot)`:

- one nearby land-purchase event chooses a random character response from bank
  `0`, slots `44–46`;
- one nearby upgrade-or-sell event chooses a random character response from
  bank `0`, slots `49–50`.

The evidence is explicit machine code: the first call asks the random helper
for a value below `3` and adds `0x2C`; the second asks for a value below `2`
and adds `0x31`. Both then push bank `0` and the current character before
calling the voice helper. These are independent character response records and
remain in groups `003–008`; they are not children of the global string pieces.

The remaining static global records have a direct mapping:

```text
global string 000/N -> voice CHARACTER/001/(N - 8), for N = 8..35
```

This does not mean every character has audio for all 28 strings. The archive's
empty markers are authoritative:

| Global strings | Voice bank 1 slots | Version 1.26 Cantonese | Version 1.18 Chinese |
| -------------- | ------------------ | ---------------------- | -------------------- |
| `008–023`      | `0–15`             | all six characters     | all six characters   |
| `024–028`      | `16–20`            | Doraemon only          | all empty            |
| `029–030`      | `21–22`            | all six characters     | all six characters   |
| `031–034`      | `23–26`            | Doraemon only          | Doraemon only        |
| `035`          | `27`               | Doraemon only          | all empty            |

Thus a global text can be shared by all characters while only one character,
or no character, has a corresponding recording in a particular release. The
Studio should show at most one bank-1 record per nonempty character root for a
global string. It must not collect unrelated bank-0 responses under the global
record.

This is a mapping limitation, not a parser limitation. Both known archives are
fully traversed to their leaves. Version 1.26 Cantonese contains all 1,140 leaf
records: 1,054 playable WAV records and 86 one-byte empty markers. Version 1.18
Chinese contains all 1,392 leaf records: 1,300 playable WAV records and 92
empty markers.

Empty markers are structural placeholders, not audio. Translation Studio does
not display them or offer replacement controls for them.

The version 1.18 Chinese archive physically contains a fourth bank of 37 leaves
per character, and the parser includes every one. Static analysis of the stock
executable found no secondary loader for it: initialization iterates exactly
three banks, the root is then released, and playback indexes only the resulting
three-bank table.

## Availability of bank 1 slots 20–27

Decoded WAV hashes were compared across all six character roots. Equality here
means byte-identical decoded audio, not merely similar speech.

In version 1.26 Cantonese:

- slot `20` and slots `23–27` contain audio only for Doraemon;
- slots `21` and `22` contain six distinct recordings, one per character.

In version 1.18 Chinese:

- slots `20` and `27` are empty for every character;
- slots `21` and `22` are identical for Doraemon, Nobita, Shizuka, Suneo, and
  Gian; Dorami has a different recording in both slots;
- slots `23–26` contain audio only for Doraemon.

These slots map to global strings `000/028–000/035`. It is not correct to infer
six recordings merely because the displayed text is global; only nonempty
physical records are playable.

## Physical record format

A leaf can contain a raw RIFF/WAVE file, a GameOne-compressed RIFF/WAVE file,
or a one-byte marker for an empty slot. The analyzed releases use `0x23` for
that marker. Existing records indicate that the game expects mono 22.05 kHz
signed 16-bit PCM. The decoded record buffer is
`0x64000` bytes, so replacement WAV files, including their headers, must not
exceed that size. This is about 9.28 seconds at the canonical format.

Rebuilding preserves untouched leaf bytes exactly. Replacements inherit the
raw or compressed convention of their original bank, and every containing
archive offset and terminal offset is recalculated. Archive paths may not be
added or removed. Empty structural slots remain hidden in Translation Studio
and are not offered as replacement targets.

## Sfx.dat

`Sfx.dat` is a separate flat GameOne archive, not another character voice
bank. Both analyzed releases contain 107 records. Every record decodes to mono
22.05 kHz, signed 16-bit PCM WAV; there are no empty or raw records in either
archive. Durations range from about 0.02 to 16.42 seconds.

Decoded-audio hashes show 103 identical records between the two releases. Only
indices `97`, `98`, `99`, and `105` differ. No decoded `Sfx.dat` record is
byte-identical to a decoded record in the newer `Voice.dat`. This supports
treating `Sfx.dat` as effects and jingles rather than as the missing storage for
shared character dialogue. Semantic names and replacement support for these
107 effects still require a separate in-game mapping pass.
