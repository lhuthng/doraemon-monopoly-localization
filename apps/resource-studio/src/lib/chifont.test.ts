import { describe, expect, test } from 'bun:test';
import { parseChiFont, validateChiFont } from '@doraemon-monopoly/dubbing-core';

function syntheticChifont(glyphs: number) {
  const data = new Uint8Array(glyphs * 32);
  for (let index = 0; index < glyphs; index += 1) {
    const record = data.subarray(index * 32, index * 32 + 32);
    record[1] = 0x01; // row 0 = 0x0001 → only column 15 is ink
    record[30] = 0x80; // row 15 = 0x8000 → only column 0 is ink
  }
  return data;
}

describe('chifont.dat', () => {
  test('validates the 32-byte atlas block size', () => {
    expect(validateChiFont(new Uint8Array(747 * 32))).toBe(747);
    expect(() => validateChiFont(new Uint8Array(33))).toThrow(/multiple of 32/);
    expect(() => validateChiFont(new Uint8Array(0))).toThrow(/multiple of 32/);
  });

  test('decodes glyphs as 16×16 row-major big-endian bitmaps', () => {
    const glyphs = parseChiFont(syntheticChifont(2));
    expect(glyphs).toHaveLength(2);
    const first = glyphs[0].pixels;
    expect(first).toHaveLength(16 * 16);
    expect(first[0]).toBe(false);
    expect(first[15]).toBe(true);
    expect(first[14]).toBe(false);
    expect(first[16]).toBe(false);
    expect(first[240]).toBe(true);
    expect(first[255]).toBe(false);
    expect(glyphs[0].index).toBe(0);
    expect(glyphs[1].index).toBe(1);
  });

  test('returns a fully transparent glyph for a zero record', () => {
    const glyphs = parseChiFont(new Uint8Array(32));
    expect(glyphs[0].pixels.every((pixel) => !pixel)).toBe(true);
  });
});
