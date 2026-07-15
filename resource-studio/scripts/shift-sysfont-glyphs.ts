import { readFileSync, writeFileSync } from 'node:fs';
import { parseSysFont, rebuildSysFont, type SysFont, type SysGlyph } from '../src/lib/formats';

function shiftGlyphDown(glyph: SysGlyph, pixels: number): SysGlyph {
  if (!Number.isInteger(pixels) || pixels < 0 || pixels >= glyph.height) {
    throw new Error(`Shift must be between 0 and ${glyph.height - 1}; got ${pixels}.`);
  }
  const shifted = new Uint8Array(glyph.pixels.length).fill(0xff);
  for (let y = 0; y < glyph.height - pixels; y += 1) {
    const source = y * glyph.width;
    shifted.set(glyph.pixels.subarray(source, source + glyph.width), (y + pixels) * glyph.width);
  }
  return { ...glyph, pixels: shifted };
}

export function shiftSysFontGlyphRange(font: SysFont, from: number, to: number, pixels: number) {
  if (!Number.isInteger(from) || !Number.isInteger(to) || from < 0 || to < from || to >= font.count) {
    throw new Error(`Invalid inclusive glyph range ${from}-${to} for ${font.count} glyphs.`);
  }
  const glyphs = [...font.glyphs];
  for (let index = from; index <= to; index += 1) glyphs[index] = shiftGlyphDown(glyphs[index], pixels);
  return { ...font, glyphs };
}

if (import.meta.main) {
  const [input, output, fromText, toText, pixelsText] = process.argv.slice(2);
  const from = Number(fromText);
  const to = Number(toText);
  const pixels = Number(pixelsText);
  if (!input || !output) {
    throw new Error('Usage: bun shift-sysfont-glyphs.ts INPUT.DAT OUTPUT.DAT FROM TO PIXELS');
  }
  const font = parseSysFont(new Uint8Array(readFileSync(input)));
  const shifted = shiftSysFontGlyphRange(font, from, to, pixels);
  writeFileSync(output, rebuildSysFont(shifted));
  console.log(`Shifted sysfont glyphs ${from}-${to} downward by ${pixels}px in ${output}.`);
}
