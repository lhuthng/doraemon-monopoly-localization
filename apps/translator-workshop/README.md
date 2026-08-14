# Translator Workshop

Public browser-based tool for contributing dialogue translations and voice
recordings to the Doraemon Monopoly localization project.

## Public contributor workflow

1. Open the Workshop at its published URL.
2. Load your own `strings.dat` and optional `voice.dat` from a legally owned
   game. Files are processed entirely in the browser.
3. Translate dialogue records and record or upload replacement voice WAVs.
4. Use **Save work** to back up your session. The modal offers:
   - **This browser** - stored locally in the browser's IndexedDB.
   - **Cloud** - zstd-compressed and stored on the project gatekeeper (needs a
     coupon; the coupon is remembered in the browser).
   - **Download ZIP** - a `doraemon-monopoly-<language>-dubbing.zip` to attach
     to a GitHub Issue. Both Local and Cloud options show when each was last
     saved and mark which one is newer.
5. Use **Load work** to restore from a ZIP file, from this browser, or from the
   cloud.

Builds that set `PUBLIC_GATEKEEPER_URL` (see `.env.example`) show a **Project
coupon** section: entering a project-issued coupon fetches the original
`strings.dat`, `voice.dat`, and `sysfont.dat` from the Cloudflare gatekeeper
instead of your own copy. The same coupon enables Cloud Save/Load work. See
[`../gatekeeper/README.md`](../gatekeeper/README.md).

## Browser privacy guarantees

- Files you load yourself are never uploaded to any server.
- The coupon download is an explicit opt-in: it fetches original files from the
  project's gatekeeper worker after you authenticate with a coupon.
- All processing happens in the browser via WebAssembly and JavaScript.
- Saved work contains only translated text and replacement audio, not
  original game files.

## Work / contribution ZIP format

The unified work file is the contribution ZIP
(`doraemon-monopoly-<language>-dubbing.zip`), so a saved session doubles as a
submission:

- `manifest.json` - format version, language, and source fingerprints.
- `dialogue/` - per-owner JSON files with translated records.
- `voices/` - per-owner WAV directories with replacement audio.

The Workshop restores its session (translations + voices) from this same ZIP,
and `make import-contribution CONTRIBUTION=...` accepts it directly. Cloud
storage sends the ZIP zstd-compressed (level 9); the gatekeeper only stores and
returns the opaque compressed blob.

## Local development

```sh
cd apps/translator-workshop
bun install
bun run dev       # start dev server
bun run build     # static build
bun run check     # type-check
```

## Shared dependency

This app depends on `@doraemon-monopoly/dubbing-core` for all canonical ID
rules, archive parsing, audio format handling, and ZIP validation. It must
never import source files directly from Resource Studio.
