# Translator Workshop

Public browser-based tool for contributing dialogue translations and voice
recordings to the Doraemon Monopoly localization project.

## Public contributor workflow

1. Open the Workshop at its published URL.
2. Load your own `strings.dat` and optional `voice.dat` from a legally owned
   game. Files are processed entirely in the browser.
3. Translate dialogue records and record or upload replacement voice WAVs.
4. Use **Save work ZIP** for a private `dubbing-work.zip` resume backup.
5. Use **Download contribution ZIP** to create a submission archive. Attach it
   to a GitHub Issue.

Builds that set `PUBLIC_GATEKEEPER_URL` (see `.env.example`) show a **Project
coupon** section: entering a project-issued coupon fetches the original
`strings.dat`, `voice.dat`, and `sysfont.dat` from the Cloudflare gatekeeper
instead of your own copy. See [`../gatekeeper/README.md`](../gatekeeper/README.md).

## Browser privacy guarantees

- Files you load yourself are never uploaded to any server.
- The coupon download is an explicit opt-in: it fetches original files from the
  project's gatekeeper worker after you authenticate with a coupon.
- All processing happens in the browser via WebAssembly and JavaScript.
- Contribution ZIPs contain only translated text and replacement audio, not
   original game files.

## Private work ZIP format

`dubbing-work.zip` contains `work.json` with the contributor's session state.
Load it back into the Workshop to resume work.

## Contribution ZIP format

`doraemon-monopoly-<language>-dubbing.zip` contains:
- `manifest.json` — format version, language, and source fingerprints.
- `dialogue/` — per-owner JSON files with translated records.
- `voices/` — per-owner WAV directories with replacement audio.

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
