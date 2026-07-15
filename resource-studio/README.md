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
```

Run `bun run translate-server` in another terminal only when local machine
translation is needed. The browser queues records one at a time; the Bun
service downloads and caches the selected Transformers.js model.

## Workspaces

| Route     | Name            | Responsibility                                                            |
| --------- | --------------- | ------------------------------------------------------------------------- |
| `/`       | String studio   | `strings.dat` decoding, editing, reflow, translation, and verified export |
| `/assets` | Graphics studio | PCX inspection and indexed Sprite1/Sprite2 PNG round-tripping             |
| `/fonts`  | Font studio     | Five `sysfont.dat` variants and numbered glyph PNG round-tripping         |

The app automatically loads `public/game/strings-CN.dat` and
`public/game/sysfont.dat`. Graphics archives are loaded on demand. These are
working copies and may already contain localization changes.

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
