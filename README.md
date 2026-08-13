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
- Windows patchers install **English**, **Vietnamese**, and a pristine
  **`<original>`** build side-by-side into the same game folder, with
  backup/restoration, local-music BGM generation and quality reduction,
  game-speed toggles, and optional compatibility extras (8-bit DirectSound,
  volume control, cnc-ddraw graphics wrapper).

## For players

Download a patcher from [GitHub Releases](https://github.com/lhuthng/doraemon-monopoly-localization/releases).
Copy `patcher.exe` beside your own `Doraemon.exe`, run it, tick the builds you
want — **`<original>`**, **English**, and/or **Vietnamese** — pick any
compatibility extras, and press **Apply patch**. The patcher validates the
installation against the public fingerprints, backs up files before changing
them, and creates a restore tool.

Languages are installed **side-by-side**: applying English and Vietnamese writes
`Doraemon-en.exe` and `Doraemon-vi.exe` (each with its own icon and localized
resources like `sprite1-en.dat`), leaving your original `Doraemon.exe` untouched.
Tick **`<original>`** to also rebuild a pristine, unpatched `Doraemon.exe` for
anything that needs the exact stock game. Tick nothing and apply to return the
game to its original files.

**Restore backup** (or `backup/Restore.exe`) puts back the exact files saved
before the last patch. **Skip disc check** and **Skip registry check** only
bypass validation; they do not provide missing game or CD data. **Use local
music** replaces CD/MCI music with a generated `BGM.dat`; the **Reduce BGM.dat
/ Reduce Voice.dat** + **Quality** controls shrink it further. **Game speed**
scales the normal in-game speed (the fast setting is unaffected). **Add
graphics wrapper** is an optional compatibility feature, separate from
translation. **Use 8-bit DirectSound output** keeps 22,050 Hz stereo playback
but selects the SB16 8-bit DMA path; it is off by default, and a local-music
stream follows the same path when both are on. **Play** launches the build you
picked. Hover any dashed option underline for a short explanation.

Keep an untouched copy of the game. The patcher works beside the executable and
does not need a path to another installation, and each action appends to a
`Doraemon-Patcher-diagnostic.log` in the game folder.

## Translator Workshop

The public [Translator Workshop](https://github.com/lhuthng/doraemon-monopoly-localization)
lets contributors load their own `strings.dat` and optional `voice.dat` locally
in the browser. Files you load yourself never enter a ZIP or leave your device,
and are not included in downloads.

Workshop builds that set `PUBLIC_GATEKEEPER_URL` additionally show a **Project
coupon** section: contributors who hold a project-issued coupon can fetch the
original `strings.dat`, `voice.dat`, and `sysfont.dat` from the project's
Cloudflare gatekeeper instead of loading their own copy. See
[`apps/gatekeeper/`](apps/gatekeeper/README.md).

**Save work** and **Load work** each open a modal with Local and Cloud options
(plus a download/from-file option). Local keeps a copy in the browser;
Cloud stores a zstd-compressed copy on the gatekeeper behind the coupon. Both
options show when they were last saved and mark which one is newer.

There is a single work/contribution ZIP type:

- `doraemon-monopoly-<language>-dubbing.zip` contains a manifest, translated
  dialogue JSON, and replacement WAV files. The Workshop restores its session
  from it, and maintainers can import it into the canonical source tree.

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
| `apps/gatekeeper/` | Optional Cloudflare Worker + R2 gatekeeper for fetching private game files. |
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

The data flow:

```text
      contributor ZIP
           │ 1. import-contribution
           ▼
  content/dubbing/<lang>   ←────────────────┐
   (canonical source)                       │ 3. export-dubbing
           │                                │   (Studio edits back up)
           │ 2. apply-dubbing               │
           ▼                                │
  local-game/<lang>  ───────────────────────┘
   (Studio workspace, make studio-en/vi)
           │ 4. build-patch
           ▼
  content/patches/<lang>   (.dmpatch components)
           │ 5. build-patcher
           ▼
  workspace/release/patcher.exe  ──►  players

  auto catalogue regen (apply/export/import/build-patch)
  or manual: make update-catalogue
           ▼
  generated-dubbing-catalogue.zst  ──►  Translator Workshop site
```

Everything starts from `workspace/base/` (private originals) via `make prepare`,
which materializes the `local-game/` Studio workspaces. `content/dubbing/` is the
canonical, reviewable source; the Translator Workshop catalogue is generated from
it automatically by the commands above. Do not edit `.dmpatch` files directly —
rebuild them from their source files.

## Maintainer workflow

### 1. Prepare the machine

Install **rustup** (recommended; the repo's `rust-toolchain.toml` pins Rust 1.77.2
and the `x86_64-pc-windows-gnu` target, which rustup installs automatically on
first build — the distro-packaged `cargo`/`rustc` ignores that file and only ships
the Linux host target):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Then install **Bun** and the MinGW cross-toolchains. Both are needed when
building Windows binaries: the 32-bit one compiles the BGM runtime
(`game-patch` build script), the 64-bit one links and resource-stamps the
patcher (`x86_64-pc-windows-gnu`):

```sh
sudo apt-get install -y gcc-mingw-w64-i686 gcc-mingw-w64-x86-64   # Debian/Ubuntu
sudo dnf install mingw32-gcc mingw64-gcc                           # Fedora
brew install mingw-w64                                             # macOS
```

`make check-mingw` verifies the cross-toolchains; `make dependencies` installs
the Bun workspace and fetches the Rust crates.

Place these files from an untouched game in `workspace/base/`:

```text
Doraemon.exe strings.dat sysfont.dat Sprite1.dat sprite2.dat bitmaps.dat voice.dat
```

Authorized maintainers can skip the manual copy and fetch the same files through
the project gatekeeper (a second, optional way — manual setup stays primary):

```sh
make fetch-base       # reads CLOUDFLARE_GATEKEEPER_URL / CLOUDFLARE_GATEKEEPER_SECRET
```

Every download is verified against the public SHA-256 fingerprints in
`content/base-fingerprints.json`.

#### Gatekeeper flow (optional, second path)

The gatekeeper (`apps/gatekeeper/`) is a Cloudflare Worker + R2 bucket that serves
the private base files to authorized callers. Full setup lives in its
[README](apps/gatekeeper/README.md); the flow is:

```text
Terraform (bucket + KV + worker) -> wrangler secrets -> R2 upload -> coupons -> Workshop fetch
```

- **Infra**: `cd apps/gatekeeper && bun run build`, then `terraform apply`
  (create the R2 bucket, KV namespace, and worker script + bindings).
- **Secrets** (`wrangler secret put`, never in files or git):
  - `MAINTAINER_SECRET` — used by `make fetch-base` (mirrored in
    `apps/resource-studio/.env` and `apps/gatekeeper/.dev.vars`).
  - `COUPON_HASHES` — JSON array of coupon SHA-256 digests for the Workshop.
    Managed by the coupon commands below (they push it automatically); no manual
    `wrangler secret put` needed.
- **Upload**: `make upload-base` pushes the 7 base files into R2 with sha256
  metadata (needs `R2_*` keys in `apps/resource-studio/.env`).
- **Coupons**: the local `apps/gatekeeper/coupons.registry.json` (gitignored)
  is the single source of truth. `make gatekeeper-add-coupon COUPON="Phrase"`
  (or without `COUPON=` for a random one) mints a coupon and pushes the worker
  secret immediately — every command needs `CLOUDFLARE_API_TOKEN` (and
  `CLOUDFLARE_ACCOUNT_ID`) in `apps/gatekeeper/.env`, so minting is always
  live, never a manual step. `make gatekeeper-list-coupons` shows active vs
  revoked; `make gatekeeper-delete-coupon COUPON=...` (or `HASH=...`) revokes
  one and re-pushes; `make gatekeeper-sync-coupons` force-pushes the current
  active set. Any human-readable string works as a coupon.
- **Workshop**: builds with `PUBLIC_GATEKEEPER_URL` (from a repo variable in the
  deploy workflow) show a **Project coupon** field that fetches
  `strings.dat`, `voice.dat`, and `sysfont.dat` from the worker. The same coupon
  powers Cloud **Save/Load work**: the Workshop zstd-compresses its work ZIP and
  the worker stores/returns the opaque compressed blob under
  `work/coupon/<sha256(coupon)>.zst` in the bucket.

Install dependencies, then materialize the current component patches over the
original game into ignored local workspaces:

```sh
make dependencies
make prepare
```

`make prepare` creates the `local-game/` workspaces Studio edits; it does not
apply or overwrite canonical dubbing. From here, two main flows keep every output
(Studio, Translator site, and release patches) in sync:

### 2. Flow A — Import a contribution ZIP into everything

Run these in order. Each command regenerates the Translator Workshop catalogue, so
the translator always reflects the newest translation:

```sh
# 1. Validate + merge the ZIP into canonical content/dubbing/vietnamese/
#    (creates a timestamped backup in workspace/dubbing-import-backups/)
make import-contribution CONTRIBUTION=workspace/<contribution>.zip

# 2. Push the imported dubbing into the Studio workspace, so studio-vi shows it
make apply-dubbing LANGUAGE=vietnamese

# 3. Optional: review or adjust it in the browser
make studio-vi

# 4. Export (no-op here, already in sync) and build all three tracked components
make build-patch LANGUAGE=vietnamese PUBLISH=1

# 5. Commit and push → the Translator site redeploys automatically
git add content/dubbing content/patches apps/resource-studio/src/lib/generated-dubbing-catalogue.zst
git commit -m "Import contribution <name>"
git push
```

What each step updates:

| Step | Updates |
| --- | --- |
| 1. `import-contribution` | `content/dubbing/vietnamese/` + Translator catalogue |
| 2. `apply-dubbing` | Studio workspace `local-game/vietnamese/` (so Studio shows it) |
| 4. `build-patch` | Tracked release components `content/patches/vietnamese/` |
| 5. commit + push | Live Translator Workshop site |

### 3. Flow B — Edit in Studio, then update everything else

Studio edits live in the generated workspace until you export them. New files you
add under `local-game/<language>/` (sprites, fonts, etc.) are picked up by the
component build automatically:

```sh
# 1. Launch Studio on the prepared workspace and make your edits
make studio-vi

# 2. Pull dialogue + voice edits back into canonical content/dubbing/vietnamese/
#    (also regenerates the Translator catalogue)
make export-dubbing LANGUAGE=vietnamese

# 3. Build all three tracked components → content/patches/vietnamese/
make build-patch LANGUAGE=vietnamese PUBLISH=1

# 4. Commit and push → the Translator site redeploys automatically
git add content/dubbing content/patches apps/resource-studio/src/lib/generated-dubbing-catalogue.zst
git commit -m "Studio edits for Vietnamese"
git push
```

### 4. Validate

Run the complete local checks before committing:

```sh
make check
```

### 5. Build and release a patcher

To package the tracked components into a Windows patcher locally:

```sh
make build-patcher
```

The result is `workspace/release/patcher.exe`. Copy it beside a test game's
`Doraemon.exe` on Windows 11, apply the selected language, test Play/Restore,
and verify local music and wrapper options when relevant.

To publish it as a GitHub release, push a tag:

```sh
git tag patcher-v1.x.x
git push origin patcher-v1.x.x
```

The `release-language.yml` workflow builds `patcher.exe` from the tracked
`content/patches/` components and uploads it to GitHub Releases.

Notes:

- `PUBLISH=1` writes tracked `content/patches/`. Without it, component output goes
  to ignored `workspace/patches/` for a candidate build. `build-dubbing`,
  `build-sprites`, and `build-runtime` each build a single component if you need
  finer control.
- The Translator Workshop catalogue (`generated-dubbing-catalogue.zst`) is
  regenerated automatically by `import-contribution`, `apply-dubbing`, and
  `export-dubbing` (and therefore `build-patch`). Commit it together with the
  `content/dubbing/` changes so the deploy picks them up.
- `make release` checks that all language components exist and builds a local
  patcher without publishing anything.

## Make command reference

Run `make help` for the same workflow in the terminal.

| Command | Purpose |
| --- | --- |
| `make check` | Run Rust tests plus Resource Studio and Workshop checks/tests. |
| `make dependencies` | Install locked Bun dependencies and fetch Rust crates (installs the pinned Rust toolchain + target via rustup). |
| `make check-mingw` | Verify the 32-bit and 64-bit MinGW cross-toolchains are installed. |
| `make prepare` | Materialize local-game from workspace/base and current patches only. |
| `make fetch-base` | Optional: fetch workspace/base files from the gatekeeper worker. |
| `make upload-base` | Upload workspace/base files into the gatekeeper's R2 bucket. |
| `make gatekeeper-mint` | Mint a random coupon and print its SHA-256 digest. |
| `make gatekeeper-add-coupon COUPON=...` | Mint (or use a given) coupon, record it, and push to Cloudflare immediately. |
| `make gatekeeper-sync-coupons` | Force-push the current active coupon set from the registry. |
| `make gatekeeper-list-coupons` | List coupons and whether they're active or revoked. |
| `make gatekeeper-delete-coupon COUPON=...` | Revoke a coupon (or `HASH=<sha256>` for legacy ones). |
| `make apply-dubbing LANGUAGE=...` | Apply canonical dubbing to one prepared Studio workspace (also regenerates the Workshop catalogue). |
| `make export-dubbing LANGUAGE=...` | Pull Studio edits back into canonical content/dubbing (also regenerates the Workshop catalogue). |
| `make import-contribution CONTRIBUTION=...` | Import and validate a Workshop contribution ZIP (also regenerates the Workshop catalogue). |
| `make update-catalogue` | Regenerate the Translator Workshop catalogue from `content/dubbing/`. |
| `make studio-en` / `make studio-vi` | Launch an existing Studio workspace without preparing it. |
| `make build-dubbing LANGUAGE=... PUBLISH=1` | Export Studio dubbing, then build the dubbing component. |
| `make build-sprites LANGUAGE=... PUBLISH=1` | Build the graphics component. |
| `make build-runtime LANGUAGE=... PUBLISH=1` | Build the runtime component. |
| `make build-patch LANGUAGE=... PUBLISH=1` | Export Studio dubbing, build all components, and regenerate the Workshop catalogue. |
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

The optional gatekeeper worker can serve a private copy of the original
resources to explicitly authorized people. That is still distribution of
copyrighted material, so it is gated behind secrets/coupons and never the
default path; see [`apps/gatekeeper/`](apps/gatekeeper/README.md).

cnc-ddraw is redistributed under its included MIT license. See
[`vendor/cnc-ddraw/LICENSE`](vendor/cnc-ddraw/LICENSE) and the
[upstream project](https://github.com/FunkyFr3sh/cnc-ddraw).
