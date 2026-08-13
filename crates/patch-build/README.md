# patch-build

Payload generation, verification, and Windows patcher packaging for the Doraemon
Monopoly localization release pipeline. Every command reads several WORKSPACE
inputs and rejects ambiguous or unsupported base files, so candidate and tracked
outputs never silently diverge.

Build the release patcher with:

```sh
cargo run -p patch-build -- universal \
  --english-payload-dir content/patches/english \
  --vietnamese-payload-dir content/patches/vietnamese \
  --output-dir workspace/release \
  --cnc-ddraw-dir vendor/cnc-ddraw
# -> workspace/release/patcher.exe (+ .sha256, README.txt)
```

`make build-patcher` wraps this. Run `patch-build` with no arguments for the
short usage text.

## Language payloads

`content/patches/<language>/` holds three canonical components built by
`release-parts`:

| Component | Contains |
| --- | --- |
| `dubbing.dmpatch` | `strings.dat` / `voice.dat` dialogue work (contributor owned). |
| `sprites.dmpatch` | `sysfont.dat`, `Sprite1.dat`, `sprite2.dat`, `bitmaps.dat`. |
| `runtime.dmpatch` | The `Doraemon.exe` placeholder plus bundled runtime files when built with `--cnc-ddraw-dir`; the PE hooks themselves are applied at install time. |

Each is a `DMPT` part blob; a language payload is their merge. Legacy monolithic
(`DMPATCH4`/`DMPATCH5`) and old multipart (`loc-*.dmpatch`) inputs stay readable
for migration.

## Subcommands

| Command | Flags | What it does |
| --- | --- | --- |
| `release-parts` | `--language english\|vietnamese` `--base-dir` `--target-dir` `--output-dir` `[--target all\|dubbing\|sprites\|runtime]` | Build the tracked `.dmpatch` components from a base game and a localized target tree. Default is `all` (dubbing + sprites + runtime). |
| `merge-parts` | `--parts-dir` `--output PATCH.dmpatch` | Merge every `.dmpatch` part in a parts directory into one monolithic `.dmpatch`. |
| `materialize` | `--payload` `--base-dir` `--output-dir` | Verify a monolithic payload against a base folder, then rebuild the patched local game files (`strings.dat`, file patches, voice, sysfont) into `output-dir`. |
| `materialize-file` | `--payload` `--base-dir` `--file` `--output` | `materialize` for a single resource (e.g. `strings.dat`). |
| `materialize-parts` | `--parts-dir` `--base-dir` `--output-dir` | `materialize` using a parts directory instead of a monolithic payload. |
| `universal` | `--output-dir` `(--english-payload\|--english-payload-dir)` `(--vietnamese-payload\|--vietnamese-payload-dir)` `[--cnc-ddraw-dir]` | Build the single Windows patcher that installs `<original>`, English, and Vietnamese side-by-side. Language inputs can be monolithic payloads or parts directories; `--cnc-ddraw-dir` bundles the graphics wrapper (validated for `ddraw.dll`, `ddraw.ini`, `cnc-ddraw config.exe`). Outputs `patcher.exe` + `patcher.exe.sha256` + `README.txt`. |
| `package` | `--output-dir` `--payload` | Build `Doraemon-<Language>-Patcher.exe` (single-language monolithic). |
| `release` | `--language` `--base-dir` `--target-dir` `--output-dir` `[--payload-only]` | Legacy single-language release as `Doraemon-<Language>-Patcher.exe`; `--payload-only` writes the `.dmpatch` without building the EXE. |
| `portable` | `--output-dir` | Build `Doraemon-Portable-Patcher.exe`: compatibility-only (Custom language, no localizable resources) with the cnc-ddraw wrapper built in through the runtime component. |
| `directsound-8bit` | `--input` `--output` | Write a guarded 8-bit DirectSound executable to `--output` without touching `--input`; refuses equal paths. |
| `vi-font` | `--input` `--output` | Extend a 640-glyph sysfont to the 1,920-glyph Vietnamese layout, writing a separate output file. **Bootstrap only**: the generated variant-0 glyphs stamp marks at guessed positions and most are incorrect — hand-correct them (Font Studio PNG export/import) before release. |
| `extract-audio` | `--cue` `--output` | Extract a verified CUE/BIN disc image into a local-music source archive. |

Cargo flags forwarded to the Windows build (`--target`, `--release`) are also
accepted by the packaging commands. Outputs are read back and verified where a
bit flip is easy to miss (executables, `BGM.dat`, packaged patches).

## Compiling payloads into the patcher

The patcher's `build.rs` reads these environment variables (also documented in
`crates/patcher/build.rs`):

| Variable | Meaning |
| --- | --- |
| `DORAEMON_PATCH_PARTS_ENGLISH` / `DORAEMON_PATCH_PARTS_VIETNAMESE` | Parts directories whose `dubbing` / `sprites` / `runtime` files become a zstd-19 `DPART` bundle embedded in the EXE. |
| `DORAEMON_PATCH_PAYLOAD_ENGLISH` / `DORAEMON_PATCH_PAYLOAD_VIETNAMESE` | Monolithic payload files embedded as fallback payloads. |
| `DORAEMON_PATCH_PAYLOAD` | Monolithic payload for single-language builds. |

On `x86_64-pc-windows-gnu` the patcher EXE is resource-stamped with the 32-bit
language icon. Nothing on the Windows target requires a running game; all
verification is fingerprint-based.

## Publishing

Pushing a `patcher-v*` tag runs `release-language.yml`, which rebuilds the parts
bundle from `content/patches/`, runs `universal` with `--cnc-ddraw-dir
vendor/cnc-ddraw`, and attaches `release/patcher.exe` to the GitHub release.