import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { parseStrings, parseSysFont, rebuildStrings, rebuildSysFont } from './formats';
import {
  extendSysFont,
  EXTENDED_SYSFONT_GLYPHS,
  VIETNAMESE_CHARACTERS,
  vietnameseBytes,
  vietnameseGlyphIndex,
  vietnameseSlot
} from './vietnamese-font';

const gameFile = (name: string) =>
  new Uint8Array(readFileSync(new URL(`../../public/game/${name}`, import.meta.url)));

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
    const extended = extendSysFont(parseSysFont(gameFile('sysfont.dat')));
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

describe('strings.dat Vietnamese encoding', () => {
  test('round-trips mixed Vietnamese, ASCII, Chinese archive records', () => {
    const original = gameFile('strings-CN.dat');
    const records = parseStrings(original);
    const id = records[0].id;
    const text = `Cánh cửa thần kỳ Đã`;
    const rebuilt = rebuildStrings(original, records, { [id]: text });
    const record = parseStrings(rebuilt).find((candidate) => candidate.id === id)!;
    expect(record.tokens.filter((token) => token.type === 'vietnamese')).toHaveLength(6);
    expect(record.tokens.some((token) => token.type === 'ascii')).toBe(true);
  });
});
