# Contributing translations and voices

## Contributors

Use the public Translator Workshop to load your own `strings.dat` and optional
`voice.dat`. The files stay in the browser. They are never uploaded and are not
included in exported ZIPs.

Use **Save work ZIP** for a private `dubbing-work.zip` backup that you may load
back into the Workshop later. Use **Download contribution ZIP** when submitting
work to the project. Attach that contribution ZIP to a GitHub Issue; do not
attach original game files.

## Maintainers

Put an untouched, legally obtained game in `workspace/base/` and import a
contribution without manually copying every file:

```sh
make import-contribution CONTRIBUTION=workspace/doraemon-monopoly-vietnamese-dubbing.zip
```

The importer validates the manifest and supported base fingerprints, checks
dialogue IDs and owners, checks voice IDs and WAV format, merges the
contribution into `content/dubbing/<language>/`, and creates a timestamped
backup in `workspace/dubbing-import-backups/`. It never commits original game
files and does not publish a patch automatically.

Prepare a local editing/build workspace when needed:

```sh
make prepare
make apply-dubbing LANGUAGE=vietnamese
make studio-vi       # or make studio-en
```

The canonical source is `content/dubbing/`.
`apps/resource-studio/local-game/` is generated and ignored. Use **Sync from
dubbing** and **Save to dubbing** in Resource Studio when working
interactively.

Before committing source changes:

```sh
cd apps/resource-studio
bun run dubbing:organize vietnamese
bun run dubbing:check vietnamese
cd ../..
make check
```

Build a tracked component only when it is ready to review:

```sh
make build-dubbing LANGUAGE=vietnamese PUBLISH=1
```

Sprites, fonts, bitmaps, and maps stay in their Resource Studio workflows. The
map view is currently read-only inspection. Do not edit generated `.dmpatch`
files directly.
