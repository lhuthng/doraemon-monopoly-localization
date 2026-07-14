import type { Palette } from './asset-formats';

export type IndexedPng = {
  width: number;
  height: number;
  pixels: Uint8Array;
  alpha: Uint8Array;
  palette: Palette;
};

const signature = Uint8Array.of(137, 80, 78, 71, 13, 10, 26, 10);
const encoder = new TextEncoder();
const decoder = new TextDecoder();

function u32be(data: Uint8Array, offset: number) {
  return ((data[offset] * 0x1000000) + (data[offset + 1] << 16) + (data[offset + 2] << 8) + data[offset + 3]) >>> 0;
}

function putU32be(data: Uint8Array, offset: number, value: number) {
  data[offset] = (value >>> 24) & 0xff;
  data[offset + 1] = (value >>> 16) & 0xff;
  data[offset + 2] = (value >>> 8) & 0xff;
  data[offset + 3] = value & 0xff;
}

function crc32(data: Uint8Array) {
  let crc = 0xffffffff;
  for (const byte of data) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function join(parts: Uint8Array[]) {
  const output = new Uint8Array(parts.reduce((length, part) => length + part.length, 0));
  let offset = 0;
  for (const part of parts) { output.set(part, offset); offset += part.length; }
  return output;
}

function chunk(type: string, payload: Uint8Array) {
  const name = encoder.encode(type);
  const output = new Uint8Array(12 + payload.length);
  putU32be(output, 0, payload.length);
  output.set(name, 4);
  output.set(payload, 8);
  putU32be(output, 8 + payload.length, crc32(output.subarray(4, 8 + payload.length)));
  return output;
}

async function transform(data: Uint8Array, stream: CompressionStream | DecompressionStream) {
  const output = new Response(stream.readable).arrayBuffer();
  const writer = stream.writable.getWriter();
  await writer.write(data);
  await writer.close();
  return new Uint8Array(await output);
}

export function transparencyIndex(pixels: Uint8Array, alpha: Uint8Array) {
  const used = new Uint8Array(256);
  for (let index = 0; index < pixels.length; index += 1) if (alpha[index]) used[pixels[index]] = 1;
  const available = used.findIndex((value) => value === 0);
  if (available < 0) throw new Error('Sprite uses all 256 indices and has no free transparent palette slot.');
  return available;
}

export async function encodeIndexedPng(image: IndexedPng, transparent: number) {
  if (image.width < 1 || image.height < 1 || image.pixels.length !== image.width * image.height || image.alpha.length !== image.pixels.length) {
    throw new Error('Indexed PNG dimensions do not match its pixels.');
  }
  if (image.palette.length !== 768 || transparent < 0 || transparent > 255) throw new Error('Indexed PNG requires a 256-color palette and one transparent index.');
  const scanlines = new Uint8Array((image.width + 1) * image.height);
  for (let row = 0; row < image.height; row += 1) {
    const target = row * (image.width + 1);
    scanlines[target] = 0;
    for (let x = 0; x < image.width; x += 1) {
      const pixel = row * image.width + x;
      scanlines[target + 1 + x] = image.alpha[pixel] ? image.pixels[pixel] : transparent;
    }
  }
  const header = new Uint8Array(13);
  putU32be(header, 0, image.width);
  putU32be(header, 4, image.height);
  header.set([8, 3, 0, 0, 0], 8);
  const alpha = new Uint8Array(256); alpha.fill(255); alpha[transparent] = 0;
  const compressed = await transform(scanlines, new CompressionStream('deflate'));
  return join([signature, chunk('IHDR', header), chunk('PLTE', image.palette), chunk('tRNS', alpha), chunk('IDAT', compressed), chunk('IEND', new Uint8Array())]);
}

function paeth(left: number, above: number, upperLeft: number) {
  const estimate = left + above - upperLeft;
  const leftDistance = Math.abs(estimate - left);
  const aboveDistance = Math.abs(estimate - above);
  const diagonalDistance = Math.abs(estimate - upperLeft);
  return leftDistance <= aboveDistance && leftDistance <= diagonalDistance ? left : aboveDistance <= diagonalDistance ? above : upperLeft;
}

export async function decodeIndexedPng(data: Uint8Array): Promise<IndexedPng> {
  if (data.length < signature.length || !signature.every((byte, index) => data[index] === byte)) throw new Error('Not a PNG file.');
  let position = signature.length;
  let width = 0, height = 0;
  let palette: Uint8Array | undefined;
  let transparency = new Uint8Array();
  const compressed: Uint8Array[] = [];
  let sawHeader = false, sawEnd = false;

  while (position + 12 <= data.length) {
    const length = u32be(data, position);
    if (length > data.length - position - 12) throw new Error('PNG chunk extends beyond the file.');
    const typeBytes = data.subarray(position + 4, position + 8);
    const type = decoder.decode(typeBytes);
    const payload = data.subarray(position + 8, position + 8 + length);
    const expectedCrc = u32be(data, position + 8 + length);
    if (crc32(data.subarray(position + 4, position + 8 + length)) !== expectedCrc) throw new Error(`PNG ${type} chunk has an invalid checksum.`);
    if (type === 'IHDR') {
      if (length !== 13 || sawHeader) throw new Error('PNG has an invalid IHDR chunk.');
      width = u32be(payload, 0); height = u32be(payload, 4); sawHeader = true;
      if (!width || !height || payload[8] !== 8 || payload[9] !== 3) throw new Error('Replacement must be an 8-bit indexed PNG, not RGB or RGBA.');
      if (payload[10] || payload[11] || payload[12]) throw new Error('Compressed, filtered, or interlaced PNG variants are not supported.');
    } else if (type === 'PLTE') {
      if (!length || length > 768 || length % 3) throw new Error('PNG has an invalid palette.');
      palette = payload.slice();
    } else if (type === 'tRNS') transparency = payload.slice();
    else if (type === 'IDAT') compressed.push(payload.slice());
    else if (type === 'IEND') { sawEnd = true; break; }
    position += 12 + length;
  }
  if (!sawHeader || !sawEnd || !palette || !compressed.length) throw new Error('PNG is missing required indexed-image chunks.');
  const paletteEntries = palette.length / 3;
  const inflated = await transform(join(compressed), new DecompressionStream('deflate'));
  const stride = width + 1;
  if (inflated.length !== stride * height) throw new Error('PNG scanline data has an unexpected size.');
  const pixels = new Uint8Array(width * height);
  for (let row = 0; row < height; row += 1) {
    const filter = inflated[row * stride];
    if (filter > 4) throw new Error(`PNG uses unknown filter ${filter}.`);
    for (let x = 0; x < width; x += 1) {
      const source = inflated[row * stride + 1 + x];
      const left = x ? pixels[row * width + x - 1] : 0;
      const above = row ? pixels[(row - 1) * width + x] : 0;
      const upperLeft = row && x ? pixels[(row - 1) * width + x - 1] : 0;
      const predictor = filter === 1 ? left : filter === 2 ? above : filter === 3 ? Math.floor((left + above) / 2) : filter === 4 ? paeth(left, above, upperLeft) : 0;
      const value = (source + predictor) & 0xff;
      if (value >= paletteEntries) throw new Error(`PNG pixel references missing palette index ${value}.`);
      pixels[row * width + x] = value;
    }
  }
  const alpha = new Uint8Array(pixels.length); alpha.fill(255);
  for (let index = 0; index < pixels.length; index += 1) {
    const value = pixels[index] < transparency.length ? transparency[pixels[index]] : 255;
    if (value !== 0 && value !== 255) throw new Error('Replacement PNG contains partial transparency; sprites require binary transparency.');
    alpha[index] = value;
  }
  const fullPalette = new Uint8Array(768); fullPalette.set(palette);
  return { width, height, pixels, alpha, palette: fullPalette };
}
