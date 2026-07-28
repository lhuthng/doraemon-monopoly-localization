# @doraemon-monopoly/dubbing-core

Shared browser-safe TypeScript primitives for the Doraemon Monopoly
localization project.

## Package contract

- Canonical record IDs and ownership rules.
- GameOne archive parsing and rebuilding.
- String and voice format encoding/decoding.
- ZIP creation and validation.
- Chi-font glyph mapping and text layout.

## Consumers

- **Resource Studio** — browser-based editors.
- **Translator Workshop** — public contribution tool.
- **CLI tooling** — dubbing import, check, and sync scripts.

## Rule

Applications must import this package; they must not import one another's
source files directly. Shared primitives belong here.
