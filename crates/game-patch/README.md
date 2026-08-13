# doraemon-game-patch

Semantic archive patching, executable (PE) patching, and audio handling used to
build and apply Doraemon Monopoly localization payloads. It is the engine behind
the [patcher UI](../patcher/README.md) and [`patch-build`](../patch-build/README.md).

## Modules

| Module | Role |
| --- | --- |
| `payload` | Declarative patch payloads and the on-disk container formats. |
| `pe` | Executable patching: language runtime, compatibility hooks, DirectSound, game clock. |
| `strings` | Semantic dialogue patches against the `strings.dat` dialogue tree. |
| `sysfont` | Glyph-table surgery for the Vietnamese font (`sysfont.dat`). The generated variant-0 glyphs are **bootstrap art only** — see below. |
| `voice` | Transcoding the WAV leaves inside `voice.dat` at a chosen quality. |
| `music` / `cue` | Re-encoding CD/MCI music into a local `BGM.dat` (WAV or CUE/BIN source). |
| `install` | Applying payloads to a real game: backup, restore, split side-by-side builds. |
| `delta` | Content-addressed binary patching (`DMD1` diff blocks) that keeps payloads small. |
| `hash` | The SHA-256 fingerprints that gate every application and backup check. |

## Payload format

A payload is a `DMPATCH5` (legacy `DMPATCH4`) blob whose parts carry three kinds
of patches:

- **File patches** — head drops and `delta` patches, keyed to a base fingerprint
  (SHA-256 of the original file). Applying requires a matching base file.
- **Strings patches** — dialogue changes applied using the `strings.dat` tree.
- **Voice and sysfont patches** — voice replacement leaves and Vietnamese glyph
  slots.

The release pipeline tracks three canonical components per language as
`content/patches/<lang>/*.dmpatch`:

- `runtime.dmpatch` — marks the `Doraemon.exe` base fingerprint (v1.26/v1.18)
  and carries any bundled runtime files (e.g. the cnc-ddraw wrapper). The actual
  PE hooks are emitted by the installer at apply time from the payload language
  plus the selected options — they are not stored as bytes in the part.
- `sprites.dmpatch` — `sysfont.dat`, `Sprite1.dat`, `sprite2.dat`, `bitmaps.dat`.
- `dubbing.dmpatch` — `strings.dat`, `voice.dat` dialogue work (contributor owned).

A `DPART` blob — `DPART` magic, a u16 part count, an offset/length table, then
the `DMPT` part blobs — packs several components into one container, and a
single patcher EXE carries English and Vietnamese this way. `patch-build
universal` compiles the two languages' DPART blobs into one zstd-19 `DZC1`
bundle (16 bytes of little-endian length headers plus a single compressed
stream), so the shared runtime component is compressed only once.

## Executable patching (`pe`)

Every hook is written as a guarded, structural rewrite that refuses ambiguous
input:

- **Language runtime** — re-routes the game's font/cache/music to localized
  resources. English patches the runtime directly; Vietnamese hooks the extended
  sysfont slots as well.

**Vietnamese glyphs are bootstrap art.** `sysfont::extend` synthesizes the
variant-0 Vietnamese glyphs by copying the de-accented ASCII base and stamping
accent marks at guessed positions; most of the results are visibly incorrect.
They are a seed for hand-editing (Font Studio exports a variant as numbered
PNGs, which artists fix and re-import), never final art — correct them before a
release.
- **Compatibility** — no-disc (`StaticFileLoadA` on the CD path), no-registry,
  local-music streaming (`BGM.dat`), and a modern volume hook for Windows 7+ /
  CrossOver.
- **8-bit DirectSound** — rewrites the primary sound parameters to 22,050 Hz /
  44,100 bytes/sec / 2-byte blocks / 8-bit samples for the SB16 path.
- **Game clock** — rewrites the single multimedia-timer period byte (stock 33 ms)
  so the tick-based game runs at the requested speed without touching the in-game
  fast setting.

## Audio

- `music.rs` reads a valid `BGM.dat`, a verified `DoraemonMusic.wav`, or a
  CUE/BIN disc image and produces a new local-music `BGM.dat` (exactly ten
  tracks). Quality variants re-cook the files at a smaller bitrate.
- `voice.rs` rebuilds a `voice.dat` archive keeping only its standard PCM WAV
  leaves transcoded to **Original** (22,050/16-bit), **High** (22,050/8-bit),
  **Balanced** (11,025/16-bit), or **Compact** (11,025/8-bit); unsupported layouts
  pass through unchanged.

## Installing and restoring

`install` implements two application modes plus full restore:

- **Single-language apply** — patches `Doraemon.exe` in place, backs originals
  into `backup/original/`, and records `backup/manifest.json` (with any generated
  files, e.g. `BGM.dat`). Its `restore`/`restore_skipping`/`compressed_audio_files`
  helpers drive the keep-compressed-audio re-apply flow.
- **Split apply** (`apply_split`) — installs English/Vietnamese and/or a
  pristine `<original>` build side-by-side. Changed files get lower-cased
  suffixed names (`sprite1-vi.dat`), executables become `Doraemon-en.exe` /
  `Doraemon-vi.exe` (resource references rewritten, per-language icon), and a
  `backup/split-manifest.json` records created and removed names so
  `restore_split` puts the exact originals back.
- `restore_any` / `has_restorable_backup` decide whether a folder has a restorable
  backup and restore every marker found (`split-manifest.json` and/or
  `manifest.json`), so a split install and a coexisting single-language backup are
  both rolled back in one call.
- `add_wrapper` copies the bundled cnc-ddraw wrapper and refuses to overwrite a
  pre-existing different file.

## Using it from code

```rust
use doraemon_game_patch::payload::{merge_parts, decode_part};
use doraemon_game_patch::install::{self, ApplyOptions};

let part = decode_part(&std::fs::read("dubbing.dmpatch")?)?;
let payload = merge_parts(&[part])?;
let report = install::apply(
    game_dir,
    &payload,
    &ApplyOptions { no_disc: true, no_reg: true, ..Default::default() },
    patcher_exe,
)?;
assert!(!report.changed.is_empty());
install::restore(&game_dir.join("backup"))?;
```

Run the tests with `cargo test -p doraemon-game-patch`; the fixtures build real
split builds (`Doraemon-en.exe`, suffixed resources) and round-trip them.