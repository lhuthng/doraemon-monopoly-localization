# Resource Studio

A Svelte 5 and Bun application for inspecting and rebuilding user-supplied
_Doraemon Monopoly_ resources locally in the browser. No game file is bundled
into development or production builds.

## Commands

```sh
bun install
bun run dev
bun run check
bun run test
bun run lint
bun run build
```

Run `bun run translate-server` in another terminal only when local machine
translation is needed.

## Workspaces

| Route     | Workspace          | Inputs                                                                    |
| --------- | ------------------ | ------------------------------------------------------------------------- |
| `/`       | Translation studio | `strings.dat`, `voice.dat`; optional `sysfont.dat` for width-aware reflow |
| `/assets` | Graphics studio    | `bitmaps.dat`, `Sprite1.dat`, and `sprite2.dat`                           |
| `/fonts`  | Font studio        | `sysfont.dat` and optional numbered PNG replacements                      |

Use the file controls or route-specific drop zones. Optional ignored local
copies may be placed under `public/game/` with the exact canonical filenames.
A missing optional file is an empty state, not an application error.

## Editing guarantees

- Untranslated string records preserve their original decoded bytes.
- String group and child offsets rebuild dynamically.
- Leading and trailing spaces remain intact; line breaks encode as `\N`.
- Voice replacements normalize to mono 22.05 kHz 16-bit PCM WAV and preserve untouched packed records.
- Indexed sprite imports preserve palette indices rather than canvas RGB.
- Sprite1 preserves its hotspot when resized; Sprite2 has no hotspot fields.
- Untouched archive records remain byte-for-byte unchanged.
- Font Studio can extend a 640-record sysfont to the five Vietnamese CC/CD
  banks without changing the filename.

The TypeScript in this project is the browser editor and optional translation
service. Executable patching, font release generation, backup/restore, and disc
audio extraction live in the root Rust workspace.
