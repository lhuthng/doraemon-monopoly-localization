# doraemon-patcher

Native [Slint](https://slint.dev/) GUI patcher that applies language and
compatibility patches to Doraemon Monopoly on Windows. It runs beside the game's
`Doraemon.exe`, validates the installation against the payload's base
fingerprints, backs up every original it would change, and can restore the exact
originals afterwards.

The GUI is Windows-only. Build the same patcher from Linux or macOS with
[`patch-build universal`](../patch-build/README.md) - it only produces the EXE.

## What Apply does

- **Restores first.** Every `Apply patch` starts by restoring the files from the
  previous run, then reinstalls your selections. Applying twice is safe and
  idempotent. (The engine's `keep_compressed_audio` option can skip restoring the
  compressed `BGM.dat`/`voice.dat` instead; the GUI does not currently expose
  it.)
- **Split, side-by-side installs.** Tick **English** and/or **Vietnamese** and the
  patcher writes suffixed builds: `Doraemon-en.exe` / `Doraemon-vi.exe`, each with
  its own icon, plus a suffixed copy of every file that language changes
  (`sprite1-en.dat`, `strings-en.dat`, `bitmaps-vi.dat` … - lower-cased, suffix
  before the extension). Files neither language touches remain shared. Each
  executable's resource references are rewritten to its suffixed names.
- **`<original>` build.** Tick **`<original>`** to keep an unpatched, in-place
  `Doraemon.exe` rebuilt from the pristine backups - stock speed, CD music, no
  hooks - for anything that needs the untouched executable.
- **Nothing ticked** returns the game to its pristine original files.

## Options

| Option | Effect |
| --- | --- |
| **Languages** | Install **English**, **Vietnamese**, and/or **`<original>`** side-by-side. |
| **Skip disc check** / **Skip registry check** | Bypass CD/registry validation only; they never supply missing game or CD data. |
| **Use local music** | Generates `BGM.dat` from an existing valid `BGM.dat`, a verified `DoraemonMusic.wav`, or a valid CUE/BIN disc image, and installs the local-music runtime. No usable source present → the option is skipped and original CD/MCI playback is untouched. |
| **Fix volume control (Windows 7+ / CrossOver)** | Adds a working in-game volume slider on modern Windows / CrossOver. |
| **Use 8-bit DirectSound output (Windows 95 / v86)** | Pins 22,050 Hz stereo, 44,100 bytes/sec, 2-byte blocks, 8-bit samples - the SB16 8-bit DMA path. Local-music streams follow the same path. Off by default. |
| **Game speed** | **Normal** (stock 33 ms tick), **1.5×** (22 ms), **2×** (17 ms), **3×** (11 ms), **4×** (8 ms). The game counts multimedia-timer ticks instead of measuring time, so this scales the *normal* speed only; the in-game fast setting is unaffected. |
| **Reduce BGM.dat** | Re-cooks `BGM.dat` at the selected quality from the verified `DoraemonMusic.wav` or CUE/BIN source. **Balanced**/**Compact** BGM needs the newer local-music runtime, so a disc-loader `BGMRT3` executable must be updated with one local-music apply first. |
| **Reduce Voice.dat** | Re-transcodes the WAV leaves inside `voice.dat` at the selected quality. Only standard PCM leaves are rebuilt; unsupported WAV layouts are kept unchanged. |
| **Quality** | **Original** (22,050 Hz / 16-bit, pass-through), **High** (22,050 / 8-bit), **Balanced** (11,025 / 16-bit), **Compact** (11,025 / 8-bit). Used by both BGM and voice reduction. |
| **Compress** | Applies the audio reductions and quality to the in-place `Doraemon.exe` without restoring anything first and without touching the language profiles. Run it again after applying a language, because applying a language restores the originals (and with them any compressed audio). |
| **Add graphics wrapper** | Installs the bundled cnc-ddraw wrapper (`ddraw.dll` and friends) and offers to open its config tool; refuses to overwrite a pre-existing different file. Only present when built with `--cnc-ddraw-dir`. |
| **Play** | Launches the installed game. With more than one executable in the game folder it asks which one first. |
| **Restore backup** | Returns the game to the exact originals saved in `backup/`. |

The dashed-underline hints in the UI describe each option. **Refresh** rescans the
folder for a valid local-music source.

## Backup and restore

- `backup/original/` holds a pristine copy of every original the patcher touched.
- A single-language apply records `backup/manifest.json` (originals + created
  files, e.g. a generated `BGM.dat`); a split apply records
  `backup/split-manifest.json` (created and removed file names).
- `backup/Restore.exe` is a copy of the patcher that runs in restore mode
  because its own filename is `Restore.exe`: it restores the folder holding it.
  Rename it and it works as a normal patcher again.
- A restore reinstates the originals and deletes the files the patch created - a
  full round trip. It handles a split backup, a single-language backup, or both
  at once (whichever `backup/` holds) and removes each marker it used.

## Diagnostics

Apply, restore, and audio actions append to `Doraemon-Patcher-diagnostic.log` in
the game folder, recording the running / done / failed state. The log is reset at
the start of each apply.

## Building

The patcher embeds the language payloads (a `DPART` bundle, zstd-19 compressed,
which shrank the release EXE by about 55%):

```sh
cargo run -p patch-build -- universal \
  --english-payload-dir content/patches/english \
  --vietnamese-payload-dir content/patches/vietnamese \
  --output-dir workspace/release \
  --cnc-ddraw-dir vendor/cnc-ddraw
```

or `make build-patcher`. The result is `workspace/release/patcher.exe`. See
[`crates/patch-build`](../patch-build/README.md).

## Windows verification checklist

Use a throwaway copy of a real game on Windows 11. `patcher.exe` must sit beside
`Doraemon.exe`.

1. **Split install.** Tick **English** + **Vietnamese**, leave **`<original>`**
   off, press **Apply patch**. Verify `Doraemon-en.exe` and `Doraemon-vi.exe`
   appear with distinct icons, with suffixed resources (`sprite1-en.dat`,
   `sprite1-vi.dat`, `strings-en.dat`, `voice-vi.dat` …). In-game, the English
   build shows English, the Vietnamese build Vietnamese; hover the underlines for
   hints.
2. **No-overlap.** Apply the same side-by-side set twice. The second apply
   restores first and reinstalls without growing the file set, and the manifest
   stays consistent.
3. **`<original>` build.** Add **`<original>`** and apply. `Doraemon.exe` plays
   unmodified (stock speed, CD music, no hooks) while `Doraemon-en.exe` stays
   localized. Untick it, re-apply, confirm `Doraemon.exe` reverts.
4. **Original-only.** Untick everything, apply. Files return to pristine and the
   final log entry is done.
5. **Restore round trip.** Run `backup/Restore.exe`. Verify every suffixed copy
   and language executable is gone and the originals are back; then apply again -
   restore + reapply must round-trip cleanly.
6. **Compressed audio.** Tick **Reduce Voice.dat**, pick **Compact**, press
   **Compress**, and confirm `voice.dat` shrinks and still replays. Then tick a
   language and apply - because apply restores originals first, verify the voice
   is back to stock afterwards, and run **Compress** once more to re-reduce it.
7. **Local music.** Put a valid `DoraemonMusic.wav` beside the game, tick **Use
   local music**, apply, and confirm `BGM.dat` plays. Then tick **Reduce
   BGM.dat** + **Compact**: the first attempt may ask for a plain local-music
   apply first (the disc-loader marker is still in the executable); afterwards the
   smaller `BGM.dat` must take.
8. **8-bit DirectSound.** Tick **Use 8-bit DirectSound output**, verify 22,050 Hz
   stereo, 44,100 bytes/sec, 2-byte blocks, 8-bit samples. Repeat with **Use local
   music** to confirm the BGM stream follows the same 8-bit path.
9. **Game speed.** Pick **4x faster**, play, and verify the *normal* speed
   animates proportionally faster while the in-game *fast* setting is unchanged.
   Set **Normal**, re-apply, confirm the stock speed returns without a restore.
10. **Volume.** With **Fix volume control** applied, confirm the in-game volume
    slider works on Windows 11 and under CrossOver if available.
11. **Play picker.** With several executables present, **Play** opens the picker
    and launches the choice; with one, it runs directly.
12. **Wrapper.** If bundled, **Add graphics wrapper** writes `ddraw.dll` and
    friends and offers the config tool; a pre-existing different `ddraw.dll` is
    rejected without overwrite. A full restore removes it.
13. **Diagnostics.** Confirm `Doraemon-Patcher-diagnostic.log` exists and its
    last entry reflects the final apply.