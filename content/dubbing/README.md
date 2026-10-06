# Dubbing source

`content/dubbing/` is the editable, shareable source for dialogue translations
and replacement voice recordings. Run
`cd apps/resource-studio && bun run dubbing:organize` before committing, and
use `bun run dubbing:check` to validate the tree.

The Studio owns sprites, fonts, bitmaps, and maps. Do not place those resources
here.

## Voice tips (any TTS engine)

Replacement WAVs must be mono 22.05 kHz 16-bit PCM within the game cache
(0x64000 bytes); the packer normalizes chunk layout to `fmt, data`
automatically, since the 1998 parser does not skip leading metadata.

- **Group codes**: bank-1 slots registered to Doraemon but shared by all
  characters: `016`, `020`, `021`, `022` (menu/action/misc), `023`–`027`
  (misc, Doraemon-only recordings). Record one line per slot per character
  and mix, or leave the original.
- **Group delta rule**: when mixing per-character takes of one shared line,
  check durations first; beyond ~1.0s spread across the six, re-render the
  outlier instead of merging.
- **Strays**: detect structurally, not by ear or STT: 30ms energy windows,
  a burst detached by >=0.20s silence is a TTS artifact. Trim at the
  previous burst end + 0.05s.
