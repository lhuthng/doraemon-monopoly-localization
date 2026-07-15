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
const putU16 = (data: Uint8Array, offset: number, value: number) => {
  data[offset] = value & 0xff;
  data[offset + 1] = (value >>> 8) & 0xff;
};

function spriteRowOffsets(data: Uint8Array, header: number, height: number) {
  const offsets: number[] = [];
  let wrap = 0;
  let previous = -1;
  for (let row = 0; row < height; row += 1) {
    const raw = u16(data, header + row * 2);
    if (raw <= previous) wrap += 0x10000;
    offsets.push(raw + wrap);
    previous = raw;
  }
  return offsets;
}

export function diagnosticPalette(): Palette {
  const palette = new Uint8Array(256 * 3);
  for (let index = 0; index < 256; index += 1) {
    const hue = (index * 137.508) % 360;
    const chroma = 0.72;
    const light = index < 16 ? 0.18 + index * 0.035 : 0.56;
    const x = chroma * (1 - Math.abs(((hue / 60) % 2) - 1));
    const sector = Math.floor(hue / 60);
    const [r, g, b] = (
      [
        [chroma, x, 0],
        [x, chroma, 0],
        [0, chroma, x],
        [0, x, chroma],
        [x, 0, chroma],
        [chroma, 0, x]
      ] as number[][]
    )[sector];
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
  const palette =
    paletteOffset >= 128 && data[paletteOffset] === 0x0c
      ? data.slice(paletteOffset + 1)
      : diagnosticPalette();
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
  for (let row = 0; row < height; row += 1)
    pixels.set(decoded.subarray(row * bytesPerLine, row * bytesPerLine + width), row * width);
  return { kind: 'bitmap', id: entry.id, width, height, pixels, palette, byteLength: data.length };
}

export function parseSprite(entry: GameOneArchiveEntry): IndexedImage | undefined {
  const data = entry.data;
  if (!data || data.length < 8) return;
  const magic = u16(data, 0);
  const width = u16(data, 2);
  const height = u16(data, 4);
  const fixedHeaderLength = magic & 1 ? 10 : 6;
  if (
    !(magic & 0x8000) ||
    !(magic & 2) ||
    width < 1 ||
    height < 1 ||
    width > 2048 ||
    height > 2048 ||
    fixedHeaderLength + height * 2 > data.length
  )
    return;
  const pixels = new Uint8Array(width * height);
  const alpha = new Uint8Array(width * height);
  const rowOffsets = spriteRowOffsets(data, fixedHeaderLength, height);
  for (let row = 0; row < height; row += 1) {
    let position = fixedHeaderLength + rowOffsets[row];
    const nextRow = row + 1 < height ? fixedHeaderLength + rowOffsets[row + 1] : data.length;
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
  return {
    kind: 'sprite',
    id: entry.id,
    width,
    height,
    pixels,
    alpha,
    hotspotX: magic & 1 ? i16(data, 6) : undefined,
    hotspotY: magic & 1 ? i16(data, 8) : undefined,
    magic,
    byteLength: data.length
  };
}

export function encodeSprite(image: IndexedImage) {
  if (
    image.kind !== 'sprite' ||
    !image.magic ||
    ((image.magic & 1) !== 0 && (image.hotspotX === undefined || image.hotspotY === undefined))
  ) {
    throw new Error(`Sprite ${image.id} is missing its original format metadata.`);
  }
  if (image.width < 1 || image.height < 1 || image.width > 2048 || image.height > 2048) {
    throw new Error(`Sprite ${image.id} has unsupported dimensions ${image.width}×${image.height}.`);
  }
  if (image.pixels.length !== image.width * image.height || image.alpha?.length !== image.pixels.length) {
    throw new Error(`Sprite ${image.id} has inconsistent pixel or transparency data.`);
  }
  const rows: Uint8Array[] = [];
  for (let row = 0; row < image.height; row += 1) {
    const payload: number[] = [];
    let x = 0;
    while (x < image.width) {
      const visible = image.alpha[row * image.width + x] !== 0;
      let end = x + 1;
      while (end < image.width && (image.alpha[row * image.width + end] !== 0) === visible) end += 1;
      const length = end - x;
      const command = visible ? length : (0x10000 - length) & 0xffff;
      payload.push(command & 0xff, command >>> 8);
      if (visible)
        for (let column = x; column < end; column += 1)
          payload.push(image.pixels[row * image.width + column]);
      x = end;
    }
    if (payload.length > 0xffff) throw new Error(`Sprite ${image.id} row ${row} is too large to encode.`);
    const encoded = new Uint8Array(payload.length + 2);
    putU16(encoded, 0, payload.length);
    encoded.set(payload, 2);
    rows.push(encoded);
  }

  const fixedHeaderLength = image.magic & 1 ? 10 : 6;
  const headerLength = fixedHeaderLength + image.height * 2;
  const total = headerLength + rows.reduce((length, row) => length + row.length, 0);
  const output = new Uint8Array(total);
  putU16(output, 0, image.magic);
  putU16(output, 2, image.width);
  putU16(output, 4, image.height);
  if (image.magic & 1) {
    putU16(output, 6, image.hotspotX! & 0xffff);
    putU16(output, 8, image.hotspotY! & 0xffff);
  }
  let cursor = headerLength;
  for (let row = 0; row < rows.length; row += 1) {
    putU16(output, fixedHeaderLength + row * 2, cursor - fixedHeaderLength);
    output.set(rows[row], cursor);
    cursor += rows[row].length;
  }
  return output;
}

export function readBitmaps(data: Uint8Array) {
  const entries = extractGameOneArchive(data);
  return { entries, images: entries.map(parsePcx).filter((image): image is IndexedImage => Boolean(image)) };
}

export function readSprites(data: Uint8Array) {
  const entries = extractGameOneArchive(data);
  return {
    entries,
    images: entries.map(parseSprite).filter((image): image is IndexedImage => Boolean(image))
  };
}
