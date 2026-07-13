# Doraemon Monopoly string translator

This standalone Svelte 5 app decodes `strings.dat` into selectable Traditional
Chinese text using a built-in, context-checked map of the game's custom glyph
IDs. Font files are reference data only and are never loaded or modified. The
selected `strings.dat` stays local to your computer.

## Run it

```sh
cd glyph-viewer
bun install
bun run dev
```

Open the local address Vite prints in the terminal.

## Build static HTML

```sh
cd glyph-viewer
bun run build
```

The static site is written to `glyph-viewer/dist/`. It does not bundle game
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

**Translate all Vietnamese** runs the direct Chinese-to-Vietnamese
`Xenova/m2m100_418M` model inside the browser with Transformers.js.
The first run downloads the model into the browser cache. Record IDs and
newlines remain controlled, Vietnamese accents are removed for the ASCII game
font, and the built-in Doraemon terminology overrides are applied afterward.
