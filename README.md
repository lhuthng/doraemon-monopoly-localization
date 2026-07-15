# Doraemon Monopoly localization research

Reverse-engineering notes and a Svelte 5 resource studio for GameOne's 1998
Windows 95/98 _Doraemon Monopoly_. The project can inspect, edit, rebuild, and
verify the game's string, sysfont, bitmap, Sprite1, and Sprite2 resources.

## Repository layout

```text
.
├── resource-studio/           Svelte 5 + Bun application
│   ├── public/game/           bundled working copies used by the browser
│   ├── scripts/               local translation service
│   └── src/
│       ├── features/          strings, fonts, and graphics workspaces
│       ├── lib/               binary codecs and shared browser utilities
│       └── styles/            global and graphics-specific styles
└── docs/
│   ├── file-formats.md        current binary-format reference
│   ├── reverse-engineering-journal.md
│   └── sprite-localization-catalog/
```

`resource-studio/public/game/` contains the app's current working copies. Some
of those files include localization edits and must not be mistaken for pristine
game archives.

## Run the studio

```sh
cd resource-studio
bun install
bun run dev
```

| Route     | Workspace       | Capabilities                                                          |
| --------- | --------------- | --------------------------------------------------------------------- |
| `/`       | String studio   | Decode, translate, reflow, import, and rebuild `strings.dat`          |
| `/assets` | Graphics studio | Inspect bitmaps; export, replace, resize, and rebuild Sprite1/Sprite2 |
| `/fonts`  | Font studio     | Inspect, export, replace, and rebuild `sysfont.dat` glyphs            |

Machine translation is optional and runs in a second terminal:

```sh
cd resource-studio
bun run translate-server
```

## Quality checks

```sh
cd resource-studio
bun run check    # Svelte and TypeScript diagnostics
bun run lint     # ESLint and formatting verification
bun run build    # production build
```

## Documentation

- [Binary file-format reference](docs/file-formats.md)
- [Chronological executable/debugger investigation](docs/reverse-engineering-journal.md)
- [Sprite1 localization catalogue](docs/sprite-localization-catalog/README.md)

The repository contains no tracked game executable. Always test rebuilt files
against a copied installation or disk image.
