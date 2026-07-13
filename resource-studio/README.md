# Doraemon Monopoly resource studio

This Svelte 5 application inspects and rebuilds GameOne string archives and
displays the game's bitmap and transparent sprite resources. Selected files
stay local to the computer.

## Run it

```sh
cd resource-studio
bun install
bun run dev
```

Open the local address Vite prints in the terminal.

| Route | Tool |
| --- | --- |
| `/` | String archive inspector/editor |
| `/assets` | `bitmaps.dat` and `Sprite1.dat` viewer |
| `/fonts` | `sysfont.dat` variant and ASCII-slot inspector |

Machine translation uses the local Bun service in a second terminal:

```sh
bun run translate-server
```

## Build static HTML

```sh
cd resource-studio
bun run build
```

The static site is written to `resource-studio/dist/`. It does not bundle game
strings; drag `strings.dat` into the page after opening it.

## Using it

Use **Load strings.dat** or drag the file onto the page. The app recreates the
executable's 14-bit decompressor, converts custom chifont IDs to Unicode, treats
the game's literal `\N` marker as a newline, and preserves real ASCII bytes.

Every source record can be copied directly. Translation text is saved in the
browser and can be imported or exported as UTF-8 JSON. **Export Chinese
records** creates `records-chinese.json`, keyed by stable IDs such as `000/000`.
**Apply translation map** accepts either `{ "000/000": "..." }`, a
`translations` mapping, or the app's project JSON, and merges matching IDs into
the editable fields. **Export strings.dat** compresses translated strings,
preserves unfinished records in their original Chinese form, rebuilds the
nested archive offsets, verifies the result, and downloads
`strings-exported.dat`.

Translation requests are processed sequentially by the local Bun service using
the selected Transformers.js model. Model files are downloaded and cached by
the server runtime. Record IDs and newlines remain controlled, and target-specific
ASCII cleanup is applied before results return to the editor.

## Generated font metrics

The width table used by pixel-aware text reflow is generated from the flattened
canonical resource at `../source/sysfont.dat`:

```sh
bun run generate:sysfont
```
