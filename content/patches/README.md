# Patches

Generated, reviewable component payloads produced from canonical source.

- `content/patches/<language>/dubbing.dmpatch` - dialogue and voice patches.
- `content/patches/<language>/sprites.dmpatch` - graphics patches.
- `content/patches/<language>/runtime.dmpatch` - executable and font patches.

These files are tracked in git for review. Do not edit them manually. Rebuild
with `make build-dubbing LANGUAGE=<lang> PUBLISH=1` and similar targets.
