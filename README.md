# Doraemon Monopoly localization research

Clean-room reverse-engineering notes, a local browser resource editor, and a
Rust patch-building system for GameOne's 1998 Windows 95/98 _Doraemon
Monopoly_. No original or rebuilt game archive, executable, disc image, or
audio track is distributed by this repository.

## Components

| Component | Purpose |
| --- | --- |
| `resource-studio/` | Svelte 5 editor for user-supplied strings, sysfonts, bitmaps, and sprites |
| `rust/game-patch/` | Verified deltas, PE patches, sysfont extension, CUE extraction, backup, and restore |
| `rust/patch-build/` | Developer CLI that turns ignored original/target directories into a Windows patcher |
| `rust/patcher/` | Native Win32 English or Vietnamese patcher GUI |
| `docs/` | File-format reference, reverse-engineering journal, and sprite index |
| `archive/` | Historical executable investigations retained as technical notes |

## Resource Studio

```sh
cd resource-studio
bun install
bun run dev
```

The Studio starts normally with `public/game/` empty. Load your own files with
the route's file buttons or drop zones. During private local development,
ignored files named `strings.dat`, `sysfont.dat`, `bitmaps.dat`, `Sprite1.dat`,
and `sprite2.dat` may be placed under `resource-studio/public/game/` for
automatic loading.

## Rust patch builder

The release builder accepts explicit directories and never searches the
repository for game data:

```sh
cargo run -p patch-build -- release \
  --language english \
  --base-dir /path/to/original \
  --target-dir /path/to/localized \
  --output-dir /path/to/release
```

Use `--language vietnamese` for the Vietnamese release. Each input directory
must contain `Doraemon.exe`, `strings.dat`, `sysfont.dat`, `Sprite1.dat`,
`sprite2.dat`, and `bitmaps.dat`. Only the exact researched Cantonese build is
accepted. Successful builds produce one self-contained Windows patcher and its
SHA-256 file; intermediate `.dmpatch` files are removed.

The patcher validates every source before writing, creates
`backup/original/`, `backup/manifest.json`, and `backup/Restore.exe`, and uses
verified replacements. Its optional no-disc mode also bypasses the legacy
Setup registry check. A user-owned CUE/BIN can be converted losslessly to
`DoraemonMusic.wav`; without valid local audio the game continues silently.

Build just the deterministic Vietnamese font extension with:

```sh
cargo run -p patch-build -- vi-font --input sysfont.dat --output sysfont.dat
```

The patched Vietnamese game continues to use the canonical names
`Doraemon.exe` and `sysfont.dat`.

## Checks

```sh
cargo test --workspace
cd resource-studio
bun run check
bun run test
bun run lint
bun run build
```

Windows releases target Rust 1.77 for Windows 7 compatibility. Cross-building
the GNU target from macOS additionally requires a MinGW-w64 linker.

## Copyright-clean workflow

Game inputs, localized full archives, patch payloads, releases, and clean-room
staging directories are ignored. Do not add them to Git. Generated patchers
contain validation metadata and binary differences that require the user's own
supported installation; they do not contain complete source archives.

