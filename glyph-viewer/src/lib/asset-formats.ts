import { extractGameOneArchive, type GameOneArchiveEntry } from './formats';

export type Palette = Uint8Array;
export type IndexedImage = {
  kind: 'bitmap' | 'sprite';
  id: string;
  width: number;
  height: number;
  pixels: Uint8Array;
  alpha?: Uint8Array;
  palette?: Palette;
  hotspotX?: number;
  hotspotY?: number;
  magic?: number;
  byteLength: number;
};

const u16 = (data: Uint8Array, offset: number) => data[offset] | (data[offset + 1] << 8);
const i16 = (data: Uint8Array, offset: number) => {
  const value = u16(data, offset);
  return value & 0x8000 ? value - 0x10000 : value;
};

export function diagnosticPalette(): Palette {
  const palette = new Uint8Array(256 * 3);
  for (let index = 0; index < 256; index += 1) {
    const hue = (index * 137.508) % 360;
    const chroma = 0.72;
    const light = index < 16 ? 0.18 + index * 0.035 : 0.56;
    const x = chroma * (1 - Math.abs(((hue / 60) % 2) - 1));
    const sector = Math.floor(hue / 60);
    const [r, g, b] = ([[chroma, x, 0], [x, chroma, 0], [0, chroma, x], [0, x, chroma], [x, 0, chroma], [chroma, 0, x]] as number[][])[sector];
    const m = light - chroma / 2;
    palette[index * 3] = Math.round(Math.max(0, Math.min(1, r + m)) * 255);
    palette[index * 3 + 1] = Math.round(Math.max(0, Math.min(1, g + m)) * 255);
    palette[index * 3 + 2] = Math.round(Math.max(0, Math.min(1, b + m)) * 255);
  }
  return palette;
}

export function parsePcx(entry: GameOneArchiveEntry): IndexedImage | undefined {
  const data = entry.data;
  if (!data || data.length < 129 || data[0] !== 0x0a || data[2] !== 1 || data[3] !== 8) return;
  const width = u16(data, 8) - u16(data, 4) + 1;
  const height = u16(data, 10) - u16(data, 6) + 1;
  const planes = data[65];
  const bytesPerLine = u16(data, 66);
  if (width < 1 || height < 1 || planes !== 1 || bytesPerLine < width) return;
  const paletteOffset = data.length - 769;
  const palette = paletteOffset >= 128 && data[paletteOffset] === 0x0c ? data.slice(paletteOffset + 1) : diagnosticPalette();
  const decoded = new Uint8Array(bytesPerLine * height);
  let source = 128;
  let target = 0;
  while (source < (paletteOffset >= 128 ? paletteOffset : data.length) && target < decoded.length) {
    const first = data[source++];
    const count = (first & 0xc0) === 0xc0 ? first & 0x3f : 1;
    const value = (first & 0xc0) === 0xc0 ? data[source++] : first;
    decoded.fill(value, target, Math.min(target + count, decoded.length));
    target += count;
  }
  if (target < decoded.length) return;
  const pixels = new Uint8Array(width * height);
  for (let row = 0; row < height; row += 1) pixels.set(decoded.subarray(row * bytesPerLine, row * bytesPerLine + width), row * width);
  return { kind: 'bitmap', id: entry.id, width, height, pixels, palette, byteLength: data.length };
}

export function parseSprite(entry: GameOneArchiveEntry): IndexedImage | undefined {
  const data = entry.data;
  if (!data || data.length < 12) return;
  const magic = u16(data, 0);
  const width = u16(data, 2);
  const height = u16(data, 4);
  if (!(magic & 0x8000) || width < 1 || height < 1 || width > 2048 || height > 2048 || 10 + height * 2 > data.length) return;
  const pixels = new Uint8Array(width * height);
  const alpha = new Uint8Array(width * height);
  for (let row = 0; row < height; row += 1) {
    let position = 10 + u16(data, 10 + row * 2);
    const nextRow = row + 1 < height ? 10 + u16(data, 10 + (row + 1) * 2) : data.length;
    if (position + 2 > data.length) return;
    const payloadLength = u16(data, position);
    position += 2;
    const limit = Math.min(position + payloadLength, nextRow, data.length);
    let x = 0;
    while (position + 2 <= limit && x < width) {
      const command = i16(data, position);
      position += 2;
      if (command < 0) x = Math.min(width, x - command);
      else if (command > 0) {
        if (position + command > limit) return;
        for (let run = 0; run < command && x < width; run += 1, x += 1) {
          const output = row * width + x;
          pixels[output] = data[position + run];
          alpha[output] = 255;
        }
        position += command;
      } else break;
    }
  }
  return { kind: 'sprite', id: entry.id, width, height, pixels, alpha, hotspotX: i16(data, 6), hotspotY: i16(data, 8), magic, byteLength: data.length };
}

export function readBitmaps(data: Uint8Array) {
  const entries = extractGameOneArchive(data);
  return { entries, images: entries.map(parsePcx).filter((image): image is IndexedImage => Boolean(image)) };
}

export function readSprites(data: Uint8Array) {
  const entries = extractGameOneArchive(data);
  return { entries, images: entries.map(parseSprite).filter((image): image is IndexedImage => Boolean(image)) };
}
