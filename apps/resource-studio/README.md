# Resource Studio

Resource Studio is a Svelte 5 and Bun application for inspecting and rebuilding
user-supplied Doraemon Monopoly resources locally. No game file is bundled into
development or production builds.

## Start here

From the repository root, put an untouched game in `workspace/base/` and run:

```sh
make dependencies
make prepare
make apply-dubbing LANGUAGE=english
make studio-en       # English workspace
make studio-vi       # Vietnamese workspace
```

`make prepare` materializes ignored `apps/resource-studio/local-game/`
workspaces from the original game and current component patches. It does not
touch dubbing. `make apply-dubbing LANGUAGE=...` is the explicit canonical
source-to-workspace step; Studio commands only launch Vite. You can also run
the package commands directly:

```sh
cd apps/resource-studio
bun install
bun run dev
bun run check
bun run test
bun run lint
bun run build
```

## Routes and inputs

| Route | Workspace | Inputs |
| --- | --- | --- |
| `/` | Translation Studio | `strings.dat`, `voice.dat`, optional `sysfont.dat` |
| `/assets` | Graphics Studio | `bitmaps.dat`, `Sprite1.dat`, `sprite2.dat` |
| `/fonts` | Font Studio | `sysfont.dat` (edit), optional `Fonts.dat`/`chifont.dat` glyph reviews |

Optional ignored copies may be placed under `public/game/` using the canonical
filenames. Missing optional files show an empty state instead of failing the
application.

## Dubbing workflow

Canonical dialogue and voice source lives in `../../content/dubbing/`.
Maintainers can import a public Workshop contribution ZIP with:

```sh
bun run dubbing:import -- ../../workspace/<contribution>.zip
```

The importer validates the ZIP, creates a backup under
`../../workspace/dubbing-import-backups/`, merges valid records into
`content/dubbing/`, and checks the result.

Useful source commands are:

```sh
bun run dubbing:organize vietnamese
bun run dubbing:check vietnamese
bun run dubbing:sync vietnamese
bun run dubbing:export vietnamese
```

`dubbing:sync` rebuilds a generated local workspace from canonical source.
`dubbing:export` moves a private local workspace back into canonical source and
should only be used intentionally.

## Editing guarantees

- Untranslated string records preserve their original decoded bytes.
- String group and child offsets rebuild dynamically.
- Leading and trailing spaces remain intact; line breaks encode as `\N`.
- Voice replacements normalize to mono 22.05 kHz 16-bit PCM WAV and preserve
  untouched packed records.
- Indexed sprite imports preserve palette indices rather than canvas RGB.
- Sprite1 preserves its hotspot when resized; Sprite2 has no hotspot fields.
- Untouched archive records remain byte-for-byte unchanged.
- Font Studio can extend a 640-record sysfont to five Vietnamese CC/CD banks.
  The generated variant-0 glyphs are **bootstrap art only** - accent marks are
  stamped at guessed positions and most are incorrect. Hand-correct them via the
  PNG export/import loop (export a variant, fix the numbered PNGs, re-import)
  before they are used in a release; never ship the generated glyphs as-is.

The Rust workspace handles executable patching, font release generation,
backup/restore, and disc-audio extraction. Resource Studio handles browser-side
resource editing and validation.

Shared primitives live in `@doraemon-monopoly/dubbing-core` and must not be
duplicated across applications.
