import type { Palette } from '../../lib/asset-formats';
import { readBitmaps } from '../../lib/asset-formats';

/** The three reusable palette bitmaps. Every other `bitmaps.dat` leaf embeds a
 * one-off artwork palette; only these three describe the colours the game
 * actually renders with and therefore qualify as review palettes. */
export const CANONICAL_PALETTE_IDS = ['000', '001', '053'] as const;
export type CanonicalPaletteId = (typeof CANONICAL_PALETTE_IDS)[number];

export type FavoritePalette = {
  id: CanonicalPaletteId;
  label: string;
  meaning: string;
  palette: Palette;
};

export const PALETTE_GUIDES: Record<CanonicalPaletteId, { label: string; meaning: string }> = {
  '000': {
    label: 'Master palette',
    meaning:
      'Bitmap #000 is the plain 640×480 main backdrop (black with white sections), yet it embeds the full 256-colour VGA set — the canonical palette every screen shares.'
  },
  '001': {
    label: 'Portrait palette',
    meaning:
      'Bitmap #001 is the Doraemon character portrait; the same 256-entry colours are shared verbatim by bitmaps #001–#006. Tuned for faces: skin tones, Doraemon red/blue and white.'
  },
  '053': {
    label: 'Title-background palette',
    meaning:
      'Bitmap #053 is the 640×480 title-screen illustration of Doraemon and friends, whose warm red/orange gradient set is the richest palette in the archive.'
  }
};

let cached: Promise<FavoritePalette[]> | undefined;

function load(): Promise<FavoritePalette[]> {
  return fetch('/game/bitmaps.dat')
    .then((response) => {
      if (!response.ok) throw new Error(`bitmaps.dat returned HTTP ${response.status}`);
      return response.arrayBuffer();
    })
    .then((bytes) => readBitmaps(new Uint8Array(bytes)))
    .then(({ images }) =>
      CANONICAL_PALETTE_IDS.filter((id) => images.some((image) => image.id === id)).map((id) => {
        const image = images.find((candidate) => candidate.id === id)!;
        return { id, ...PALETTE_GUIDES[id], palette: image.palette! };
      })
    );
}

/** Loads the canonical review palettes from the staged `bitmaps.dat` archive,
 * resolving once and reusing the result for both the Graphics and Fonts studio. */
export function loadCanonicalPalettes(): Promise<FavoritePalette[]> {
  if (!cached) cached = load();
  return cached;
}
