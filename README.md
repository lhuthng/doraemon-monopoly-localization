# Doraemon Monopoly localization

[![English](https://img.shields.io/badge/English-dialogues%20complete-2ea44f)](#current-state)
[![Vietnamese](https://img.shields.io/badge/Vietnamese-dialogues%20complete-e0a000)](#current-state)
[![Windows patchers](https://img.shields.io/badge/releases-Windows%20patchers-2563eb)](https://github.com/lhuthng/doraemon-monopoly-localization/releases)
[![License](https://img.shields.io/badge/code-MIT-blue)](#legal)

This project localizes GameOne's 1998 Windows 95/98 game **Doraemon Monopoly**.
It includes a browser-based Translator Workshop, Resource Studio editors, a
semantic archive patcher, and the tooling used to build Windows releases.

The original game, executable, disc image, music, and extracted artwork are
not distributed here. Use the tools only with game files you legally own.

## Current state

- English and Vietnamese dialogue translations are complete.
- Voice replacement support is available for the canonical dubbing sources.
- Some UI text and baked-in artwork remain untranslated.
- The patcher supports Cantonese v1.26 and v1.18 resource layouts.
- Windows patchers support language patches, backups/restoration, local music,
  legacy volume compatibility, and an optional cnc-ddraw graphics wrapper.

## For players

Download a patcher from [GitHub Releases](https://github.com/lhuthng/doraemon-monopoly-localization/releases).
Copy `patcher.exe` beside your own `Doraemon.exe`, run it, choose **English** or
**Vietnamese**, and press **Apply**. The patcher validates the installation,
backs up files before changing them, and creates a restore tool.

Use **Restore backup** to return to the files saved before the last patch.
**Skip disc check** and **Skip registry check** only bypass validation; they do
not provide missing game or CD data. **Use local music** can replace CD/MCI
music with a generated `BGM.dat`. **Add graphics wrapper** is an optional
compatibility feature, separate from translation.

Keep an untouched copy of the game. The patcher works beside the executable and
does not need a path to another installation.

## Translator Workshop

The public [Translator Workshop](https://github.com/lhuthng/doraemon-monopoly-localization)
lets contributors load their own `strings.dat` and optional `voice.dat` locally
in the browser. Original files never leave the browser and are not included in
downloads.

There are two ZIP types:

- `dubbing-work.zip` is a private resume backup. It contains `work.json` and is
  intended to be loaded back into the Workshop by the same contributor.
- `doraemon-monopoly-<language>-dubbing.zip` is a contribution export. It
  contains a manifest, translated dialogue JSON, and replacement WAV files.
  Maintainers can import it into the canonical source tree.

Maintainers can import a contribution automatically:

```sh
make import-contribution CONTRIBUTION=tmp/doraemon-monopoly-vietnamese-dubbing.zip
```

The importer checks the format, language, supported resource fingerprints,
record IDs, voice ownership, and WAV format. It creates a timestamped backup in
`tmp/dubbing-import-backups/`, merges the contribution into `dubbing/`, and
validates the result. It does not publish a patch automatically.

## Repository layout

| Path | Role |
| --- | --- |
| `tmp/base/` | Private untouched game resources used for local builds. Never commit them. |
| `dubbing/` | Canonical, reviewable dialogue and voice source. |
| `resource-studio/` | Browser editors, import/export tools, and local scripts. |
| `resource-studio/local-game/` | Ignored generated workspaces for editing and preview. |
| `patches/<language>/` | Reviewable generated `dubbing`, `sprites`, and `runtime` components. |
| `tmp/patches/` | Ignored candidate component output when `PUBLISH` is not set. |
| `tmp/release/` | Ignored local Windows patcher output. |
| `rust/game-patch/` | Archive, executable, audio, and installation logic. |
| `rust/patch-build/` | Payload generation and Windows release packaging. |
| `rust/patcher/` | Native Windows patcher UI. |
| `translator-site/` | Public Translator Workshop website. |

The source flow is:

```text
contributor ZIP -> dubbing/ -> local-game workspace -> component patches
                 -> embedded Windows patcher -> player's own game
```

Do not edit `.dmpatch` files directly. Rebuild them from their source files.

## Maintainer workflow

### 1. Prepare the machine

Install Rust/Cargo, Bun, and GNU MinGW when building Windows binaries on macOS:

```sh
brew install mingw-w64
```

Place these files from an untouched game in `tmp/base/`:

```text
Doraemon.exe strings.dat sysfont.dat Sprite1.dat sprite2.dat bitmaps.dat voice.dat
```

Then prepare ignored local workspaces:

```sh
make setup
```

`make setup` materializes the current component patches and syncs canonical
`dubbing/` into the English and Vietnamese workspaces. It does not replace the
canonical source tree.

### 2. Import or edit content

For a Workshop contribution:

```sh
make import-contribution CONTRIBUTION=tmp/<contribution>.zip
```

For local work, launch one workspace:

```sh
make studio-en
make studio-vi
```

The Translation Studio edits strings, linked dialogue voices, and voice records.
The Graphics Studio handles indexed bitmaps and sprites. Font Studio handles
glyph banks, including Vietnamese extensions. The map view is currently an
inspector rather than a map editor.

Canonical dialogue and voice edits belong in `dubbing/`. Graphics and font work
is maintained through Resource Studio and its generated local workspace.

### 3. Validate

Run the complete local checks:

```sh
make check
```

For focused dubbing checks:

```sh
cd resource-studio
bun run dubbing:organize vietnamese
bun run dubbing:check vietnamese
```

### 4. Build components

Use `PUBLISH=1` only when you intend to update tracked release components:

```sh
make build-dubbing LANGUAGE=vietnamese PUBLISH=1
make build-sprites LANGUAGE=vietnamese PUBLISH=1
make build-runtime LANGUAGE=vietnamese PUBLISH=1
```

To build all three for one language:

```sh
make build-patch LANGUAGE=vietnamese PUBLISH=1
```

Without `PUBLISH=1`, component output goes to ignored `tmp/patches/` for a
candidate build.

### 5. Package and test a patcher

After the required components exist under `patches/`:

```sh
make build-patcher
```

The result is `tmp/release/patcher.exe`. Copy it beside a test game's
`Doraemon.exe` on Windows 11, apply the selected language, test Play/Restore,
and verify local music and wrapper options when relevant.

For a local release-style check:

```sh
make release
```

This checks that all language components exist and builds the local patcher. It
does not create a GitHub release or publish files remotely.

## Make command reference

Run `make help` for the same workflow in the terminal.

| Command | Purpose |
| --- | --- |
| `make check` | Run Rust tests plus Resource Studio type checks and tests. |
| `make setup` | Generate ignored English/Vietnamese local workspaces. |
| `make import-contribution CONTRIBUTION=...` | Import and validate a Workshop contribution ZIP. |
| `make studio-en` / `make studio-vi` | Prepare and launch a Studio workspace. |
| `make build-dubbing LANGUAGE=... PUBLISH=1` | Build the dubbing component. |
| `make build-sprites LANGUAGE=... PUBLISH=1` | Build the graphics component. |
| `make build-runtime LANGUAGE=... PUBLISH=1` | Build the runtime component. |
| `make build-patch LANGUAGE=... PUBLISH=1` | Build all components for one language. |
| `make build-patcher` | Embed tracked components into `tmp/release/patcher.exe`. |
| `make release` | Check payload presence and build a local patcher. |
| `make translator-dev` | Run the public Workshop locally. |
| `make translator-build` | Build the static Workshop into `tmp/contributor-kit/`. |

## Development checks

```sh
cargo test --workspace
cd resource-studio
bun run check
bun run test
bun run lint
bun run build
```

See [`resource-studio/README.md`](resource-studio/README.md) for editor routes
and guarantees, [`CONTRIBUTING.md`](CONTRIBUTING.md) for contribution policy,
and [`dubbing/README.md`](dubbing/README.md) for canonical source rules.

## Legal

This repository contains tooling, documentation, difference payloads, and
permissively licensed compatibility files. It does not contain the original
game or replace the need for a legally obtained copy.

cnc-ddraw is redistributed under its included MIT license. See
[`third_party/cnc-ddraw/LICENSE`](third_party/cnc-ddraw/LICENSE) and the
[upstream project](https://github.com/FunkyFr3sh/cnc-ddraw).
