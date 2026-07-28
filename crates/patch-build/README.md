# patch-build

Component payload generation, candidate vs. tracked output management, and
Windows patcher packaging.

This crate handles:
- `release-parts` — generate `.dmpatch` payloads for dubbing, sprites, runtime.
- `materialize-parts` — rebuild a working local-game from patches and base.
- `universal` — package tracked components and cnc-ddraw into a patcher.exe.
