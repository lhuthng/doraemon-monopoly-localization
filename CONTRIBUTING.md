# Contributing translations and voices

Dialogue and voice work is stored in [`dubbing/`](dubbing/README.md). The source tree is intentionally small and reviewable: one sorted dialogue JSON file per owner and normalized WAV voice replacements.

Use the public Translator website. Choose a language and character, then load your own original `strings.dat` (and `voice.dat` when you want voice preview). Those files never leave your browser. The site keeps local progress on that device, can save/load a private `dubbing-work.zip` backup when moving computers, and downloads a small contribution ZIP. Attach that ZIP to a GitHub Issue; do not extract it or send original game files.

For local preview, put your legal original game files in `tmp/base/`, run `make setup`, then start `cd resource-studio && bun run dev-en` or `bun run dev-vi`. Translation Studio has **Sync from dubbing**, **Save to dubbing**, and **Check dubbing** controls in its Files & exports menu.

Before opening a pull request, run:

```sh
cd resource-studio
bun run dubbing:organize
bun run dubbing:check
```

Sprites, fonts, bitmaps, and maps stay in the relevant Resource Studio editors. Do not place them in `dubbing/`.
