# Doraemon Monopoly localization toolkit

This is a copyright-clean toolkit for researching and localizing GameOne's
1998 Windows 95/98 **Doraemon Monopoly**. It does not contain the game,
rebuilt game archives, its EXE, disc image, music, or extracted artwork.

It has two jobs:

- **Resource Studio** is a local Svelte editor for the game's strings, fonts,
  bitmaps, and two sprite archives.
- **Rust patch builders** turn your own original game and your own finished
  localization into small Windows patchers. A player supplies their own game;
  the patcher verifies it, makes a backup, then applies only the differences.

## What you need

To edit resources, obtain your own legal copy of the Cantonese release and
keep these resource files in a private, ignored folder:

```text
strings.dat
sysfont.dat
Sprite1.dat
sprite2.dat
bitmaps.dat
```

`Doraemon.exe` is needed only when building or testing a player-facing patcher.

For local music in a no-disc build, also keep the original matching CUE/BIN
beside the game, or a verified `DoraemonMusic.wav` produced from that image.
Nothing in those folders is meant to be committed.

For development, install Bun and Rust. Building Windows patchers from macOS
also needs a GNU Windows cross-linker:

```sh
brew install mingw-w64
```

The Rust workspace pins a Windows-7-compatible toolchain. Players of a
released patcher do **not** need Bun, Rust, or MinGW.

## Repository map

| Path | What it is |
| --- | --- |
| `resource-studio/` | Svelte 5 browser editor. It runs with no game files present. |
| `rust/game-patch/` | File formats, deltas, archive rebuilding, backup/restore, Vietnamese font generation, PE compatibility patches, and CUE/BIN audio extraction. |
| `rust/patch-build/` | Developer CLI that packages an English, Vietnamese, or portable Windows patcher. |
| `rust/patcher/` | Native Win32 patcher window embedded in the release EXEs. |
| `docs/` | Resource format references and reverse-engineering notes. |
| `archive/` | Historical research notes. |

## Resource Studio

Start empty and load files using each route's buttons or drop areas:

```sh
cd resource-studio
bun install
bun run dev
```

For a private working copy, create ignored folders such as:

```text
resource-studio/local-game/
├── origin/       # untouched Cantonese strings.dat
├── english/      # your English target files
└── vietnamese/   # your Vietnamese target files
```

Each language folder contains the five resource files above. Then launch a
staged workspace:

```sh
cd resource-studio
bun run dev-en
# or
bun run dev-vi
```

The command copies only private files into ignored `public/game/`. It fails
clearly when the inputs are absent; it never creates fake resources.

Useful details:

- String exports preserve game archive structure and record IDs.

## Vietnamese font extension

Vietnamese keeps the original filenames: the game still loads `Doraemon.exe`
and `sysfont.dat`. The Vietnamese patch expands `sysfont.dat` from 640 to
1,920 glyph records, then patches the game’s text routines to recognize
`CC xx` and `CD xx` as two-byte Vietnamese glyph codes. ASCII and Chinese text
continue through their original paths.

The first Vietnamese font variant contains the usable glyphs. The remaining
four banks are structurally valid blank variants for future font work. The
Font Studio exposes these banks once an extended `sysfont.dat` is loaded.

## Build a language patcher

A release is built from two private directories:

```text
base-dir                 target-dir
────────                 ──────────
untouched game           finished localization
all six files            same six filenames
```

For a player-facing patcher, the six files are the five resources plus
`Doraemon.exe`.

`base-dir` is the exact untouched Cantonese game the player is expected to
own. `target-dir` is the final localized state you edited. Copy unchanged
files from base into target as well: the builder compares each pair and embeds
only changes. It never embeds a complete game file.

```sh
cargo run -p patch-build -- release \
  --language english \
  --base-dir /private/path/to/original \
  --target-dir /private/path/to/english \
  --output-dir /private/path/to/release
```

Change `english` to `vietnamese` for the Vietnamese patcher. The output is:

```text
release/
├── Doraemon-English-Patcher.exe       # or Doraemon-Vietnamese-Patcher.exe
├── Doraemon-English-Patcher.exe.sha256
└── README.txt
```

Optional: bundle a private cnc-ddraw copy for a **Add graphics wrapper** button:

```sh
--cnc-ddraw-dir /path/to/cnc-ddraw
```

The player copies the one patcher EXE into their game folder and runs it. The
window stays open while it works. It shows a colored live log, can Apply,
Restore, add the wrapper, and launch the game with **Play**.

Apply creates `backup/original/`, `backup/manifest.json`, and
`backup/Restore.exe` before changing anything. Restore returns tracked files
to their original hashes and removes a patcher-created WAV only when it has
not been edited. A restored backup is recognized on the next Apply and is
recreated automatically, so Restore → Apply works even when music is extracted
from a CUE/BIN again.

### Versioned localization changes, without versioning game data

The shareable source of a language release is a `*.dmpatch` payload: a compact
binary delta plus file hashes and translated string records. It is **not** a
complete `.dat` archive and cannot be used without the exact supported base
game. This repository permits only these payloads under `patches/`; all other
`.dmpatch` files remain ignored.

When a maintainer has finished an English or Vietnamese target, create the
payload and commit it:

```sh
cargo run -p patch-build -- release \
  --language english \
  --base-dir /private/path/to/original \
  --target-dir /private/path/to/english \
  --output-dir patches \
  --payload-only

git add patches/english.dmpatch
git commit -m "feat(english): update localization payload"
```

This is the only step that needs both the original and finished localized
files. A fresh contributor clones the repository, supplies only their own
untouched resource files, and materializes a private Studio workspace:

```sh
cd resource-studio
bun run setup-en /path/to/their/game/folder
# or
bun run setup-vi /path/to/their/game/folder
```

The setup command verifies the original resource files in the selected folder,
stores a private original `strings.dat`, and writes only the five materialized
Studio resources to `resource-studio/local-game/english` or `vietnamese`. It
does not copy, patch, or create `Doraemon.exe`. It refuses to overwrite an
existing workspace unless you explicitly add `--force`.

## Build the portable compatibility patcher

This patcher has no language resource payload. It detects and patches the
supported executable layout at runtime:

```sh
cargo run -p patch-build -- portable \
  --output-dir /private/path/to/portable-release \
  --cnc-ddraw-dir /path/to/cnc-ddraw
```

It independently handles the old Setup-registry check, CD startup check, and
local WAV music hook. If no CUE/BIN or verified WAV is present beside the game,
the game is patched to run quietly rather than failing for lack of a disc.

## Automated GitHub releases

Keep releases free of game data. For each release, attach only:

- `Doraemon-English-Patcher.exe` **or** `Doraemon-Vietnamese-Patcher.exe`;
- its `.sha256` checksum;
- a short README/changelog.

Suggested tags and titles:

| Tag | Title | Scope |
| --- | --- | --- |
| `english-patch-v0.1.0` | English Patch v0.1.0 | Full dialogue; roughly 90% UI localization. |
| `vietnamese-patch-v0.1.0` | Vietnamese Patch v0.1.0 | Full dialogue; English UI graphics for now. |
| `portable-patch-v0.1.0` | Portable Compatibility Patch v0.1.0 | No-disc, registry, local-audio, and optional wrapper support. |

The repository includes a GitHub Actions workflow that packages a committed
language payload and creates a GitHub release automatically. It runs when you
push one of these tags:

```text
english-patch-v*
vietnamese-patch-v*
```

For example, after committing `patches/english.dmpatch`:

```sh
git tag english-patch-v0.1.0
git push origin english-patch-v0.1.0
```

GitHub builds the Windows patcher from the tracked payload, uploads the EXE
and SHA-256 checksum to the release, and generates release notes. You do not
need to build or upload those artifacts manually.

The portable compatibility patch is intentionally not automated yet. It will
get its own `portable-patch-v*` workflow after its runtime patching support is
settled.

The workflow has access only to the repository and its tracked payloads. It
never receives `.dat`, `.bin`, `.cue`, `.wav`, original EXEs, or full sprite
exports.

## Other commands

Create an extended Vietnamese font from a user-supplied sysfont:

```sh
cargo run -p patch-build -- vi-font \
  --input /path/to/sysfont.dat \
  --output /path/to/new/sysfont.dat
```

Extract a local WAV from a user-owned disc image:

```sh
cargo run -p patch-build -- extract-audio \
  --cue /path/to/DORAEMON.cue \
  --output /path/to/DoraemonMusic.wav
```

Run project checks:

```sh
cargo test --workspace
cd resource-studio
bun run check
bun run test
bun run lint
bun run build
```
