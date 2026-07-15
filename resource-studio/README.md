# Resource Studio

A Svelte 5 and Bun application for inspecting and rebuilding _Doraemon
Monopoly_ resources entirely on the local machine.

## Commands

```sh
bun install
bun run dev
bun run check
bun run lint
bun run build
bun run test
```

Run `bun run translate-server` in another terminal only when local machine
translation is needed. The browser queues records one at a time; the Bun
service downloads and caches the selected Transformers.js model.

## Workspaces

| Route     | Name            | Responsibility                                                               |
| --------- | --------------- | ---------------------------------------------------------------------------- |
| `/`       | String studio   | `strings.dat` decoding, editing, reflow, translation, and verified export    |
| `/assets` | Graphics studio | PCX inspection and indexed Sprite1/Sprite2 PNG round-tripping                |
| `/fonts`  | Font studio     | Original and Vietnamese `sysfont.dat` banks with numbered PNG round-tripping |

The app automatically loads `public/game/strings-CN.dat` and
`public/game/sysfont.dat`. Graphics archives are loaded on demand. These are
working copies and may already contain localization changes.

## Vietnamese font prototype

Font Studio expands the bundled 640-record sysfont in memory to 1,920 records.
Variant 0 contains generated Vietnamese feasibility glyphs; variants 1–4 contain
valid transparent placeholders that can be replaced using numbered PNGs.

Generate standalone test files for the known Chinese executable with:

```sh
bun run build:vi-font
bun run patch:vi-exe
```

The commands write `../tmp/sysfont-vi.dat` and `../tmp/Đô-rê-mon.exe`. The
patched executable loads the expanded font directly from `sysfont-vi.dat`, so
the original `sysfont.dat` can remain beside it unchanged. The patcher rejects
any executable whose SHA-256 does not match the analyzed build. See
[`../archive/EXECUTABLE_FONT_RESEARCH.md`](../archive/EXECUTABLE_FONT_RESEARCH.md)
for addresses, encoding, and remaining DOSBox-X verification steps.

## Source layout

```text
src/
├── features/
│   ├── strings/       string workspace and text-specific helpers
│   ├── fonts/         sysfont workspace and glyph canvas
│   └── graphics/      bitmap/sprite workspace and indexed canvas
├── lib/               GameOne/LZW, sprite, PNG, ZIP, and download utilities
├── styles/            shared and workspace-specific CSS
├── Router.svelte      three-route client router
└── main.ts            application entry point
```

`src/lib/formats.ts` owns the GameOne archive, fixed 14-bit LZW, string, and
sysfont codecs. `src/lib/asset-formats.ts` owns PCX and both sprite-header
variants. Every writer reparses and verifies its output before download.

## Editing guarantees

- Untranslated string records retain their original decoded bytes.
- String group and child offsets are rebuilt dynamically.
- Leading and trailing spaces are preserved; newlines encode as literal `\N`.
- Indexed sprite imports use palette indices rather than canvas RGB values.
- Sprite1 preserves its hotspot when resized; Sprite2 has no hotspot fields.
- Untouched archive records remain byte-for-byte unchanged.
