# Dubbing source

`content/dubbing/` is the editable, shareable source for dialogue translations
and replacement voice recordings. Run
`cd apps/resource-studio && bun run dubbing:organize` before committing, and
use `bun run dubbing:check` to validate the tree.

The Studio owns sprites, fonts, bitmaps, and maps. Do not place those resources
here.
