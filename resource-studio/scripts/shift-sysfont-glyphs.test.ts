import { expect, test } from 'bun:test';
import type { SysFont, SysGlyph } from '../src/lib/formats';
import { shiftSysFontGlyphRange } from './shift-sysfont-glyphs';

test('shifts only the inclusive glyph range downward', () => {
  const glyph = (pixel: number): SysGlyph => ({
    width: 1,
    height: 5,
    pixels: Uint8Array.from([pixel, 0xff, 0, 0xff, 0])
  });
  const font: SysFont = {
    bytes: new Uint8Array(),
    count: 3,
    variants: 1,
    glyphs: [glyph(1), glyph(2), glyph(3)]
  };
  const shifted = shiftSysFontGlyphRange(font, 1, 1, 2);
  expect([...shifted.glyphs[0].pixels]).toEqual([...font.glyphs[0].pixels]);
  expect([...shifted.glyphs[1].pixels]).toEqual([0xff, 0xff, 2, 0xff, 0]);
  expect([...shifted.glyphs[2].pixels]).toEqual([...font.glyphs[2].pixels]);
});
