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
  legacy volume compatibility, an optional Windows 95/v86 8-bit DirectSound
  mode, and an optional cnc-ddraw graphics wrapper.

## For players

Download a patcher from [GitHub Releases](https://github.com/lhuthng/doraemon-monopoly-localization/releases).
Copy `patcher.exe` beside your own `Doraemon.exe`, run it, choose **English** or
**Vietnamese**, and press **Apply**. The patcher validates the installation,
backs up files before changing them, and creates a restore tool.

Use **Restore backup** to return to the files saved before the last patch.
**Skip disc check** and **Skip registry check** only bypass validation; they do
not provide missing game or CD data. **Use local music** can replace CD/MCI
music with a generated `BGM.dat`. **Add graphics wrapper** is an optional
compatibility feature, separate from translation. **Use 8-bit DirectSound
output** keeps 22,050 Hz stereo playback but selects the SB16 8-bit DMA path;
it is off by default. When local music is also selected, its DirectSound
stream follows the same 8-bit path. Hover a dashed option underline for a
short explanation.

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
make import-contribution CONTRIBUTION=workspace/doraemon-monopoly-vietnamese-dubbing.zip
```

The importer checks the format, language, supported resource fingerprints,
record IDs, voice ownership, and WAV format. It creates a timestamped backup in
`workspace/dubbing-import-backups/`, merges the contribution into
`content/dubbing/`, and validates the result. It does not publish a patch
automatically.

## Repository layout

| Path | Role |
| --- | --- |
| `workspace/base/` | Private untouched game resources used for local builds. Never commit them. |
| `apps/resource-studio/` | Browser editors, import/export tools, and local scripts. |
| `apps/translator-workshop/` | Public Translator Workshop website. |
| `packages/dubbing-core/` | Shared TypeScript primitives (archive parsing, audio, ZIP). |
| `content/dubbing/` | Canonical, reviewable dialogue and voice source. |
| `content/patches/<language>/` | Reviewable generated `dubbing`, `sprites`, and `runtime` components. |
| `content/assets/` | Patcher/release assets (icons, etc.). |
| `crates/game-patch/` | Archive, executable, audio, and installation logic. |
| `crates/patch-build/` | Payload generation and Windows release packaging. |
| `crates/patcher/` | Native Windows patcher UI. |
| `tools/dubbing/` | Filesystem-oriented dubbing import, check, and sync tools. |
| `docs/reference/` | Stable format documentation. |
| `docs/research/` | Historical reverse-engineering notes. |
| `vendor/cnc-ddraw/` | Vendored cnc-ddraw graphics compatibility wrapper. |
| `workspace/` | Ignored local inputs, candidate patches, and generated output. |

The source flow is:

```text
contributor ZIP -> content/dubbing/ -> local-game workspace -> component patches
                 -> embedded Windows patcher -> player's own game
```

Do not edit `.dmpatch` files directly. Rebuild them from their source files.

## Maintainer workflow

### 1. Prepare the machine

Install Rust/Cargo, Bun, and GNU MinGW when building Windows binaries on macOS:

```sh
brew install mingw-w64
```

Place these files from an untouched game in `workspace/base/`:

```text
Doraemon.exe strings.dat sysfont.dat Sprite1.dat sprite2.dat bitmaps.dat voice.dat
```

Install dependencies, then materialize the current component patches over the
original game into ignored local workspaces:

```sh
make dependencies
make prepare
```

`make prepare` does not apply or overwrite canonical dubbing. Apply it
explicitly to the workspace you intend to edit:

```sh
make apply-dubbing LANGUAGE=vietnamese
```

### 2. Import or edit content

For a Workshop contribution:

```sh
make import-contribution CONTRIBUTION=workspace/<contribution>.zip
```

For local work, launch one workspace:

```sh
make studio-en
make studio-vi
```

Studio edits live in the generated local workspace until `make build-patch`
exports dialogue and voice changes back into canonical `content/dubbing/`.
Graphics and font work stays in the local workspace and is included in the
same component build.

### 3. Validate

Run the complete local checks:

```sh
make check
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

Without `PUBLISH=1`, component output goes to ignored `workspace/patches/` for a
candidate build.

### 5. Package and test a patcher

```sh
make build-patcher
```

The result is `workspace/release/patcher.exe`. Copy it beside a test game's
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
| `make check` | Run Rust tests plus Resource Studio and Workshop checks/tests. |
| `make dependencies` | Install locked Bun dependencies. |
| `make prepare` | Materialize local-game from workspace/base and current patches only. |
| `make apply-dubbing LANGUAGE=...` | Apply canonical dubbing to one prepared Studio workspace. |
| `make import-contribution CONTRIBUTION=...` | Import and validate a Workshop contribution ZIP. |
| `make studio-en` / `make studio-vi` | Launch an existing Studio workspace without preparing it. |
| `make build-dubbing LANGUAGE=... PUBLISH=1` | Export Studio dubbing, then build the dubbing component. |
| `make build-sprites LANGUAGE=... PUBLISH=1` | Build the graphics component. |
| `make build-runtime LANGUAGE=... PUBLISH=1` | Build the runtime component. |
| `make build-patch LANGUAGE=... PUBLISH=1` | Export Studio dubbing, then build all components for one language. |
| `make build-patcher` | Embed tracked components into `workspace/release/patcher.exe`. |
| `make release` | Check payload presence and build a local patcher. |
| `make translator-dev` | Run the public Workshop locally. |
| `make translator-build` | Build the static Workshop into `workspace/contributor-kit/`. |

## Development checks

```sh
cargo test --workspace
cd apps/resource-studio
bun run check
bun run test
bun run lint
bun run build
```

See each domain README for specifics:

- [`apps/resource-studio/README.md`](apps/resource-studio/README.md) — editor routes and guarantees.
- [`apps/translator-workshop/README.md`](apps/translator-workshop/README.md) — contributor workflow.
- [`packages/dubbing-core/README.md`](packages/dubbing-core/README.md) — shared package contract.
- [`content/dubbing/README.md`](content/dubbing/README.md) — canonical source rules.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — contribution policy.

## Legal

This repository contains tooling, documentation, difference payloads, and
permissively licensed compatibility files. It does not contain the original
game or replace the need for a legally obtained copy.

cnc-ddraw is redistributed under its included MIT license. See
[`vendor/cnc-ddraw/LICENSE`](vendor/cnc-ddraw/LICENSE) and the
[upstream project](https://github.com/FunkyFr3sh/cnc-ddraw).
