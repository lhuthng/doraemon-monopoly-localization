import { describe, expect, test } from 'bun:test';
import { parseSysFont, rebuildSysFont } from './formats';
import {
  extendSysFont,
  EXTENDED_SYSFONT_GLYPHS,
  VIETNAMESE_CHARACTERS,
  vietnameseBytes,
  vietnameseGlyphIndex,
  vietnameseSlot
} from './vietnamese-font';

function syntheticSysfont() {
  const count = 640;
  const tableEnd = 2 + count * 4;
  const glyphBytes = 2 + 8 * 16;
  const data = new Uint8Array(tableEnd + count * glyphBytes);
  const view = new DataView(data.buffer);
  view.setUint16(0, count, true);
  for (let index = 0; index < count; index += 1) {
    const offset = tableEnd + index * glyphBytes;
    view.setUint32(2 + index * 4, offset, true);
    data[offset] = 8;
    data[offset + 1] = 16;
    data.fill(0xff, offset + 2, offset + glyphBytes);
    data[offset + 2 + (index % 8)] = 0;
  }
  return data;
}

describe('Vietnamese codebook', () => {
  test('round-trips every supported precomposed character', () => {
    expect(VIETNAMESE_CHARACTERS).toHaveLength(134);
    for (const [slot, character] of VIETNAMESE_CHARACTERS.entries()) {
      const bytes = vietnameseBytes(character);
      expect(bytes).toBeDefined();
      expect(vietnameseSlot(bytes![0], bytes![1])).toBe(slot);
    }
  });

  test('does not claim ordinary Chinese pairs', () => {
    expect(vietnameseSlot(0x82, 0xe4)).toBeUndefined();
    expect(vietnameseSlot(0xcc, 0x80)).toBeUndefined();
  });
});

describe('extended sysfont', () => {
  test('rebuilds five structurally valid Vietnamese banks', () => {
    const extended = extendSysFont(parseSysFont(syntheticSysfont()));
    const reparsed = parseSysFont(rebuildSysFont(extended));
    expect(reparsed.count).toBe(EXTENDED_SYSFONT_GLYPHS);
    expect(reparsed.glyphs[vietnameseGlyphIndex(0, 0)].pixels.some((pixel) => pixel !== 0xff)).toBe(true);
    for (let variant = 1; variant < 5; variant += 1) {
      expect(reparsed.glyphs[vietnameseGlyphIndex(variant, 0)].pixels.every((pixel) => pixel === 0xff)).toBe(
        true
      );
    }
  });
});
