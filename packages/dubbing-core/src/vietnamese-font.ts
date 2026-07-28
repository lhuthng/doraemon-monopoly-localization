import type { SysFont, SysGlyph } from './formats';

export const VIETNAMESE_PREFIXES = [0xcc, 0xcd] as const;
export const ORIGINAL_SYSFONT_GLYPHS = 640;
export const VIETNAMESE_SLOTS_PER_VARIANT = 256;
export const VIETNAMESE_VARIANTS = 5;
export const EXTENDED_SYSFONT_GLYPHS =
  ORIGINAL_SYSFONT_GLYPHS + VIETNAMESE_SLOTS_PER_VARIANT * VIETNAMESE_VARIANTS;

const vowelFamilies = ['a', 'ă', 'â', 'e', 'ê', 'i', 'o', 'ô', 'ơ', 'u', 'ư', 'y'];
const tones = ['', '\u0300', '\u0301', '\u0309', '\u0303', '\u0323'];

export const VIETNAMESE_CHARACTERS = Object.freeze(
  [
    ...vowelFamilies.flatMap((letter) => tones.map((tone) => `${letter}${tone}`.normalize('NFC'))),
    'đ',
    ...vowelFamilies.flatMap((letter) =>
      tones.map((tone) => `${letter.toLocaleUpperCase('vi')}${tone}`.normalize('NFC'))
    ),
    'Đ'
  ].filter((character) => character.charCodeAt(0) > 0x7f)
);

if (VIETNAMESE_CHARACTERS.length !== 134) {
  throw new Error(`Expected 134 Vietnamese characters; got ${VIETNAMESE_CHARACTERS.length}.`);
}

export const VIETNAMESE_TO_SLOT = new Map(VIETNAMESE_CHARACTERS.map((character, slot) => [character, slot]));

export function vietnameseCharacter(slot: number) {
  return VIETNAMESE_CHARACTERS[slot];
}

export function vietnameseBytes(character: string): [number, number] | undefined {
  const slot = VIETNAMESE_TO_SLOT.get(character.normalize('NFC'));
  if (slot === undefined) return undefined;
  return [VIETNAMESE_PREFIXES[Math.floor(slot / 128)], slot & 0x7f];
}

export function vietnameseSlot(first: number, second: number) {
  if (second >= 0x80 || (first !== 0xcc && first !== 0xcd)) return undefined;
  const slot = (first - 0xcc) * 128 + second;
  return slot < VIETNAMESE_CHARACTERS.length ? slot : undefined;
}

export function vietnameseGlyphIndex(variant: number, slot: number) {
  return ORIGINAL_SYSFONT_GLYPHS + variant * VIETNAMESE_SLOTS_PER_VARIANT + slot;
}

function baseAscii(character: string) {
  if (character === 'Đ') return 'D';
  if (character === 'đ') return 'd';
  return character.normalize('NFD').replace(/[\u0300-\u036f]/g, '')[0];
}

function visiblePixel(value: number) {
  return value !== 0xff;
}

function generatedVariantZeroGlyph(base: SysGlyph, character: string): SysGlyph {
  const pixels = new Uint8Array(base.pixels);
  const width = base.width;
  const set = (x: number, y: number) => {
    if (x >= 0 && x < width && y >= 0 && y < base.height) pixels[y * width + x] = 0;
  };
  if (character === 'Đ' || character === 'đ') {
    const y = Math.max(1, Math.floor(base.height / 2));
    for (let x = 0; x < width; x += 1) set(x, y);
    return { width, height: base.height, pixels };
  }

  const decomposition = character.normalize('NFD').slice(1);
  const center = Math.floor(width / 2);
  let row = 0;
  for (const mark of decomposition) {
    if (mark === '\u0300') {
      set(center - 1, row);
      set(center, row + 1);
    } else if (mark === '\u0301') {
      set(center, row + 1);
      set(center + 1, row);
    } else if (mark === '\u0309') {
      set(center, row);
      set(center + 1, row);
      set(center, row + 1);
    } else if (mark === '\u0303') {
      set(center - 2, row + 1);
      set(center - 1, row);
      set(center, row + 1);
      set(center + 1, row);
    } else if (mark === '\u0323') {
      set(center, base.height - 1);
    } else if (mark === '\u0302') {
      set(center - 1, row + 1);
      set(center, row);
      set(center + 1, row + 1);
      row += 2;
    } else if (mark === '\u0306') {
      set(center - 1, row);
      set(center, row + 1);
      set(center + 1, row);
      row += 2;
    } else if (mark === '\u031b') {
      set(width - 2, Math.min(2, row));
      set(width - 1, Math.min(1, row));
      row += 1;
    }
  }
  if (!pixels.some(visiblePixel)) set(center, Math.floor(base.height / 2));
  return { width, height: base.height, pixels };
}

function placeholderGlyph(base: SysGlyph): SysGlyph {
  return {
    width: base.width,
    height: base.height,
    pixels: new Uint8Array(base.width * base.height).fill(0xff)
  };
}

export function extendSysFont(original: SysFont): SysFont {
  if (original.count === EXTENDED_SYSFONT_GLYPHS) return original;
  if (original.count !== ORIGINAL_SYSFONT_GLYPHS) {
    throw new Error(
      `Vietnamese extension requires the original ${ORIGINAL_SYSFONT_GLYPHS}-glyph sysfont; got ${original.count}.`
    );
  }
  const glyphs = [...original.glyphs];
  for (let variant = 0; variant < VIETNAMESE_VARIANTS; variant += 1) {
    for (let slot = 0; slot < VIETNAMESE_SLOTS_PER_VARIANT; slot += 1) {
      const character = vietnameseCharacter(slot) || ' ';
      const base = original.glyphs[variant * 128 + baseAscii(character).charCodeAt(0)];
      glyphs.push(
        variant === 0 && slot < VIETNAMESE_CHARACTERS.length
          ? generatedVariantZeroGlyph(base, character)
          : placeholderGlyph(base)
      );
    }
  }
  return {
    bytes: original.bytes,
    count: EXTENDED_SYSFONT_GLYPHS,
    variants: EXTENDED_SYSFONT_GLYPHS / 128,
    glyphs
  };
}
