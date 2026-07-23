# Doraemon Monopoly disc and music research

This document records verified behavior in the Chinese Windows 95/98 executable whose SHA-256 is
`fdf00e681671f93b09d257f77d7ce0720e7129cf6bc44ba9e0f19c2efa4fecba`.

## Original disc gate

Startup calls `GetLogicalDrives` at `0x0043723A`, scans drive letters C–Z, and calls the validator at
`0x004375F2`. The validator requires both:

- `<drive>:\data1.cab` with a size of at least `0x08F0D180` (150,000,000 bytes);
- `<drive>:\runme.exe`.

The accepted root is stored through the pointer at `0x004CCDF8`. The portable patch replaces the scan
at `0x0043723A`, writes the executable directory into that existing root buffer, and resumes at
`0x004372E8`. It therefore does not execute the validator or the insert-disc error path.

## Original Setup registry gate

Startup calls `0x0042CBE0`, which opens `HKEY_LOCAL_MACHINE\SOFTWARE\Gameone\Doraemon` with
`KEY_ALL_ACCESS`. If the key cannot be opened, the caller at `0x00455BE6` displays “Please install the
application from Setup” and terminates startup.

The key contains a `Data` value of type `REG_BINARY` and length 228 bytes. Importantly, the original game
already contains an initialization path for a missing or malformed `Data` value; only a missing parent key
was fatal. The portable patch replaces the failure block at `0x0042CC11` with
`mov dword ptr [ebp-0x0c],0`, then falls through to the existing value-query and initialization path. Failed
registry writes are ignored by the original code, while the initialized certificate remains available in
memory. This avoids returning success with an uninitialized output structure and removes the Setup
dependency from Wine/CrossOver and clean Windows installations.

## Original CD audio

The executable imports `mciSendCommandA` at IAT address `0x004B929C` and opens device type `0x0204`
(`MCI_DEVTYPE_CD_AUDIO`). It selects TMSF time format and addresses audio by CD track number.

| Address      | Original responsibility               | Local-music behavior                         |
| ------------ | ------------------------------------- | -------------------------------------------- |
| `0x00485043` | Open CD-audio device and select TMSF  | Open and validate `BGM.dat`                  |
| `0x00485288` | Play a numbered CD track              | Start the matching injected DirectSound stream |
| `0x00485366` | Stop CD playback                      | Stop the local DirectSound buffer            |
| `0x0048545F` | Query a CD track's duration           | Read duration from the `BGM.dat` directory  |
| `0x004855F3` | Query the physical disc's track count | Report the original 11-track numbering       |

`0x00485043` is inside the constructor that begins at `0x00485020`, not a normal function entry.
Immediately before the hook, the constructor saves its object at `[EBP-0x70]` and calls a memset-like
helper that clobbers `ECX`. The injected continuation must therefore reload the object from `[EBP-0x70]`
and return with the constructor’s original `mov esp,ebp; pop ebp; ret 4` convention. Treating `ECX` as
the object here produces a null write at `.port+0xAD` during startup.

`BGM.dat` contains ten independently decodable mono IMA ADPCM streams at 22.05 kHz, matching the format
used by the original `Sfx.dat` records. Injected code reserves a temporary SFX slot and calls the game's
memory-WAV routine at `0x00489041`, which creates the DirectSound buffer through the original, proven path.
It then takes ownership of that buffer, clears the temporary slot, and releases the reservation, so all eight
normal SFX slots remain available. A WinMM one-second periodic timer maintains a four-second looping buffer in
two halves. `0x004851D9` is redirected so shutdown kills the timer and releases the buffer and file without
issuing legacy MCI or mixer calls.

The original BGM slider targeted the Compact Disc mixer control. For local playback only, its calculated
0–65535 value is forwarded to `IDirectSoundBuffer::SetVolume`. If **Use local music** is not
selected, no local stream is installed, no music file is generated, and all original CD/MCI music and slider
instructions remain untouched. If local music is selected but no valid `BGM.dat`, WAV, or CUE/BIN source
exists, the patcher warns and likewise leaves the original music code untouched.

The patcher also offers an opt-in **Fix volume control** mode for systems that ignore the old mixer API.
It redirects the SFX slider to DirectSound-buffer volume, using a logarithmic 54-step table so halfway remains
audible at approximately -6 dB. This mode is never applied unless selected.

## Mixed-mode disc layout

The BIN contains 2352-byte sectors. Track 1 is MODE1/2352 and tracks 2–11 are CD audio. The first audio
sector is frame 102,263; the remaining 71,930 sectors are already signed 16-bit little-endian stereo PCM
at 44.1 kHz. Extraction therefore adds only a 44-byte RIFF/WAVE header and performs no transcoding.

| Track | WAV start (ms) | WAV end (ms) |
| ----: | -------------: | -----------: |
|     2 |              0 |      341,307 |
|     3 |        341,307 |      471,280 |
|     4 |        471,280 |      513,227 |
|     5 |        513,227 |      621,840 |
|     6 |        621,840 |      629,733 |
|     7 |        629,733 |      726,347 |
|     8 |        726,347 |      796,267 |
|     9 |        796,267 |      858,707 |
|    10 |        858,707 |      945,053 |
|    11 |        945,053 |      959,067 |

The extracted payload is 169,179,360 bytes and has SHA-256
`4474878caff593f35a0979d1cc94d71aef5b2ce71eac57fafb73a78419e98424`.

## Portable PE section

The patch appends `.port` at RVA `0x000D6000`/VA `0x004D6000`. It contains the startup bypasses,
local-music dispatch stubs, the embedded decoder and streamer, optional volume hooks, and title credit. The
runtime resolves `BGM.dat` beside the executable with `GetModuleFileNameA`, so playback does not depend on the
working directory. It calls only Win95-era Kernel32/WinMM imports already present in the executable and the
game's own DirectSound/SFX routines; there is no helper DLL or additional DirectSound device.

The patch addresses only disc detection and music transport. DirectDraw, DirectInput, codecs, and other
Windows 95 compatibility concerns are unchanged.
