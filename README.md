# Doraemon Monopoly localization

[![English 95%](https://img.shields.io/badge/English-95%25-2ea44f)](#localization-progress)
[![Vietnamese 50%](https://img.shields.io/badge/Vietnamese-50%25-e0a000)](#localization-progress)
[![Windows patchers](https://img.shields.io/badge/releases-Windows%20patchers-2563eb)](https://github.com/lhuthng/doraemon-monopoly-localization/releases)
[![License](https://img.shields.io/badge/code-MIT-blue)](#legal)

<p align="center">
  <img src="docs/assets/title-menu-comparison.png" alt="Original Chinese and localized English title menus" width="548">
</p>

A copyright-clean toolkit for researching and localizing GameOne's 1998
Windows 95/98 game **Doraemon Monopoly**. It provides English and Vietnamese
localization patchers, a portable compatibility patch, a Svelte resource
editor, and the reverse-engineered file tooling needed to build them.

The repository does not contain the original game, complete game archives,
disc images, music, or extracted artwork. Players and contributors must supply
their own supported Cantonese v1.26 installation.

## Downloads

Download the patcher you want from
[GitHub Releases](https://github.com/lhuthng/doraemon-monopoly-localization/releases).

| Release | What it changes | Status | Link |
| --- | --- | --- | --- |
| English Patch | English dialogues, menus, sprites, gadget descriptions, compatibility fixes, and optional local music | About 95% complete | [English releases](https://github.com/lhuthng/doraemon-monopoly-localization/releases?q=english-patch) |
| Vietnamese Patch | Vietnamese dialogues and extended Vietnamese fonts; currently inherits the English UI graphics | About 50% complete | [Vietnamese releases](https://github.com/lhuthng/doraemon-monopoly-localization/releases?q=vietnamese-patch) |
| Portable Patch | No localization. Removes the CD and Setup-registry requirements, supports local WAV music, and can install cnc-ddraw | Compatibility release | [Portable releases](https://github.com/lhuthng/doraemon-monopoly-localization/releases?q=portable-patch) |

Every release contains a Windows patcher, its SHA-256 checksum, and a short
README. It never contains a patched `Doraemon.exe` or complete `.dat` file.

### Using a patcher

1. Copy the patcher EXE into the folder containing your own `Doraemon.exe`.
2. Run the patcher and press **Apply**.
3. Review the colored task log. The window remains open until you close it.
4. Use **Add graphics wrapper** for cnc-ddraw compatibility, then **Play**.

The patcher verifies the installation, creates `backup/original/` and
`backup/manifest.json`, applies only the requested changes, and verifies every
result. **Restore** returns the tracked files to their exact original hashes.

For background music, place either a valid `DoraemonMusic.wav` or the original
matching CUE/BIN beside the game. With no audio source, the patched game still
runs but remains silent.

## Localization progress

| Language | Overall | Dialogues | UI and graphics | Notes |
| --- | ---: | --- | --- | --- |
| English | **95%** | Complete | Nearly complete | Main gameplay, menus, gadgets, and dialogue are usable in English. Some visual text may remain. |
| Vietnamese | **50%** | Complete | Uses English UI | Vietnamese text and font support are implemented. Vietnamese-specific menu and sprite artwork remains to be localized. |

These percentages describe localization coverage, not build stability. Update
them when translated resources are reviewed in game.

## What is in this repository

| Path | Purpose |
| --- | --- |
| `resource-studio/` | Svelte 5 editor for strings, fonts, bitmaps, `Sprite1.dat`, and `sprite2.dat`. |
| `rust/game-patch/` | Archive formats, semantic string patches, binary deltas, backup and restore, PE patching, Vietnamese fonts, and CUE/BIN extraction. |
| `rust/patch-build/` | Developer CLI for creating payloads and Windows patcher releases. |
| `rust/patcher/` | Native Win32 patcher interface embedded in release EXEs. |
| `patches/` | Tracked copyright-clean English and Vietnamese difference payloads. |
| `third_party/cnc-ddraw/` | Vendored MIT-licensed cnc-ddraw runtime, shaders, license, and source hashes. |
| `docs/` | File-format documentation, sprite localization catalogue, and reverse-engineering journal. |
| `archive/` | Focused executable research notes retained for reference. |
| `tmp/` | Ignored private inputs and generated output. Only `tmp/base/.gitkeep` is tracked. |

## Contributor setup

Install Bun and Rust. Building Windows patchers on macOS also requires the GNU
Windows linker:

```sh
brew install mingw-w64
```

Copy these files from your own untouched Cantonese v1.26 game into
`tmp/base/`:

```text
Doraemon.exe
strings.dat
sysfont.dat
Sprite1.dat
sprite2.dat
bitmaps.dat
```

All six files are ignored by Git. Never commit them.

Then prepare both private Studio workspaces:

```sh
make setup
```

Start the editor for one language:

```sh
cd resource-studio
bun install
bun run dev-en
# or
bun run dev-vi
```

The setup command combines your private original resources with the tracked
copyright-clean payloads. It creates ignored workspaces under
`resource-studio/local-game/`. It does not copy or create `Doraemon.exe` there.

## Make workflow

Run `make` or `make help` for the current command summary.

| Command | Result |
| --- | --- |
| `make setup` | Materializes private English and Vietnamese Studio workspaces from `tmp/base/` and the tracked payloads. |
| `make build-patch LANGUAGE=english` | Compares the English workspace with `tmp/base/` and writes an ignored candidate to `tmp/patches/english.dmpatch`. |
| `make build-patch LANGUAGE=vietnamese` | Creates the equivalent ignored Vietnamese candidate. |
| `make build-patch LANGUAGE=english PUBLISH=1` | Writes the reviewed payload to tracked `patches/english.dmpatch`. |
| `make build-patch LANGUAGE=english PATCHER=1` | Also builds a local Windows patcher in `tmp/release/` with the vendored cnc-ddraw runtime. |
| `make build-patch LANGUAGE=english PATCHER=1 CNC_DDRAW_DIR=/path/to/cnc-ddraw` | Builds with a different complete cnc-ddraw distribution. |

The payload and the patcher are different artifacts:

```text
private original files + private localized files
                    |
                    v
        patches/english.dmpatch
        copyright-clean differences
                    |
                    v
       Doraemon-English-Patcher.exe
       Rust program + embedded differences
                    |
                    v
       player's own original installation
```

Only the first step needs the maintainer's original and localized `.dat`
files. GitHub Actions can build the final patcher because the reviewed
`.dmpatch` already contains the required differences, record replacements,
validation hashes, and executable patch instructions.

## Resource Studio

The Studio can run completely empty with `bun run dev`. Optional ignored game
files may be staged into `resource-studio/public/game/`, but missing files are
treated as normal empty states.

Its main workspaces support:

- decoding, editing, reflowing, and rebuilding `strings.dat` without trimming
  intentional spaces;
- inspecting, editing, importing, and exporting `sysfont.dat` glyphs;
- decoding bitmap palettes and both sprite archive formats;
- lossless indexed-PNG sprite export and import for Aseprite;
- rebuilding GameOne archives with corrected dynamic offsets;
- server-side English and Vietnamese translation queues.

See [Resource Studio documentation](resource-studio/README.md) for its routes
and development commands.

## Vietnamese font extension

The Vietnamese patch keeps the original filenames `Doraemon.exe` and
`sysfont.dat`. It expands `sysfont.dat` from 640 to 1,920 glyph records and
patches four text routines in the executable.

The new encoding reserves two-byte sequences:

- `CC 00` through `CC 7F` for Vietnamese slots 0 through 127;
- `CD 00` through `CD 7F` for slots 128 through 255.

The executable selects glyph record
`640 + activeVariant * 256 + VietnameseSlot`. Existing ASCII and Chinese paths
remain intact. Variant 0 contains the current Vietnamese artwork; variants 1
through 4 are valid blank banks ready for future font editing.

More detail is recorded in
[Executable font research](archive/EXECUTABLE_FONT_RESEARCH.md).

## How releases are built

The GitHub Actions workflow publishes all three patcher families:

| Tag pattern | Produced artifact |
| --- | --- |
| `english-patch-v*` | `Doraemon-English-Patcher.exe` |
| `vietnamese-patch-v*` | `Doraemon-Vietnamese-Patcher.exe` |
| `portable-patch-v*` | `Doraemon-Portable-Patcher.exe` |

For example:

```sh
git tag english-patch-v0.1.0
git push origin english-patch-v0.1.0
```

GitHub compiles the Rust patcher, embeds the selected tracked payload and the
vendored cnc-ddraw files, creates a checksum, and publishes the release. The
workflow never receives the maintainer's `.dat`, original EXE, CUE, BIN, WAV,
or disc image.

You can also run the workflow manually from the
[Actions page](https://github.com/lhuthng/doraemon-monopoly-localization/actions/workflows/release-language.yml)
using an existing release tag.

## Advanced commands

Create a language payload directly:

```sh
cargo run -p patch-build -- release \
  --language english \
  --base-dir /private/path/to/original \
  --target-dir /private/path/to/english \
  --output-dir /private/path/to/output \
  --payload-only
```

Package an existing payload with cnc-ddraw:

```sh
cargo run -p patch-build -- package \
  --payload patches/english.dmpatch \
  --output-dir /private/path/to/release \
  --cnc-ddraw-dir third_party/cnc-ddraw
```

Build the portable compatibility patcher:

```sh
cargo run -p patch-build -- portable \
  --output-dir /private/path/to/release \
  --cnc-ddraw-dir third_party/cnc-ddraw
```

Create the Vietnamese font extension from a user-supplied font:

```sh
cargo run -p patch-build -- vi-font \
  --input /path/to/sysfont.dat \
  --output /path/to/new-sysfont.dat
```

Extract a local WAV from a user-owned disc image:

```sh
cargo run -p patch-build -- extract-audio \
  --cue /path/to/DORAEMON.cue \
  --output /path/to/DoraemonMusic.wav
```

## Verification

```sh
cargo test --workspace
cd resource-studio
bun run check
bun run test
bun run lint
bun run build
```

Technical references:

- [Known file formats](docs/file-formats.md)
- [Reverse-engineering journal](docs/reverse-engineering-journal.md)
- [Sprite localization catalogue](docs/sprite-localization-catalog/README.md)
- [Executable portability research](archive/EXECUTABLE_PORTABILITY_RESEARCH.md)

## Legal

This project contains original tooling, documentation, compact difference
payloads, and permissively licensed third-party compatibility files. It does
not provide the game or make a legal copy unnecessary. Use it only with files
you are entitled to use.

cnc-ddraw is redistributed under its included MIT license. See
[third_party/cnc-ddraw/LICENSE](third_party/cnc-ddraw/LICENSE) and
[upstream](https://github.com/FunkyFr3sh/cnc-ddraw).
