export type GlyphToken =
  | { type: 'end' }
  | { type: 'newline' }
  | { type: 'ascii'; byte: number; text: string }
  | { type: 'glyph'; id: number };

export type StringRecord = {
  id: string;
  path: number[];
  bytes: Uint8Array;
  tokens: GlyphToken[];
};

export type SysGlyph = {
  width: number;
  height: number;
  pixels: Uint8Array;
};

export type SysFont = {
  bytes: Uint8Array;
  count: number;
  variants: number;
  glyphs: SysGlyph[];
};

const signature = new TextEncoder().encode('\0\0GameOne Systems Limited\nWritten by Samme NG\0');

function u16(data: Uint8Array, offset: number) {
  return data[offset] | (data[offset + 1] << 8);
}

function u32(data: Uint8Array, offset: number) {
  return (
    data[offset] |
    (data[offset + 1] << 8) |
    (data[offset + 2] << 16) |
    (data[offset + 3] << 24)
  ) >>> 0;
}

function putU32(data: Uint8Array, offset: number, value: number) {
  data[offset] = value & 0xff;
  data[offset + 1] = (value >>> 8) & 0xff;
  data[offset + 2] = (value >>> 16) & 0xff;
  data[offset + 3] = (value >>> 24) & 0xff;
}

function hasSignature(data: Uint8Array, offset: number) {
  if (offset < 0 || offset + signature.length > data.length) return false;
  return signature.every((byte, index) => data[offset + index] === byte);
}

type ArchiveNode = { path: number[]; offset: number; container: boolean };

export type GameOneArchiveEntry = {
  id: string;
  path: number[];
  packed: Uint8Array;
  data?: Uint8Array;
  error?: string;
};

function archiveNodes(data: Uint8Array, offset = 0, path: number[] = []): ArchiveNode[] {
  if (!hasSignature(data, offset)) throw new Error(`Missing GameOne archive header at 0x${offset.toString(16)}.`);
  const count = u32(data, offset + 0x42);
  const table = offset + 0x66;
  if (count > 100_000 || table + count * 4 > data.length) {
    throw new Error(`Invalid archive entry count ${count}.`);
  }
  const result: ArchiveNode[] = [{ path, offset, container: true }];
  for (let index = 0; index < count; index += 1) {
    const child = offset + u32(data, table + index * 4);
    if (child >= data.length) throw new Error(`Archive entry ${[...path, index].join('/')} is outside the file.`);
    const childPath = [...path, index];
    if (hasSignature(data, child)) result.push(...archiveNodes(data, child, childPath));
    else result.push({ path: childPath, offset: child, container: false });
  }
  return result;
}

function validateArchiveStructure(data: Uint8Array, offset = 0, expectedEnd = data.length, path: number[] = []) {
  if (!hasSignature(data, offset)) throw new Error(`Missing GameOne archive header at 0x${offset.toString(16)}.`);
  const count = u32(data, offset + 0x42);
  const table = offset + 0x66;
  const tableEnd = table + (count + 1) * 4;
  if (count > 100_000 || tableEnd > data.length) {
    throw new Error(`Invalid archive entry count ${count} at ${path.join('/') || 'root'}.`);
  }

  // GameOne archives store count child offsets followed by one terminal offset.
  // The terminal entry is the container's total byte length and is required to
  // calculate the packed length of its final child.
  const terminal = offset + u32(data, table + count * 4);
  if (terminal !== expectedEnd) {
    throw new Error(
      `Archive ${path.join('/') || 'root'} ends at 0x${terminal.toString(16)}, expected 0x${expectedEnd.toString(16)}.`
    );
  }

  const childOffsets = Array.from({ length: count }, (_, index) => offset + u32(data, table + index * 4));
  for (let index = 0; index < childOffsets.length; index += 1) {
    const child = childOffsets[index];
    const end = index + 1 < childOffsets.length ? childOffsets[index + 1] : terminal;
    const childPath = [...path, index];
    if (child < tableEnd || child >= end || end > terminal) {
      throw new Error(`Archive entry ${childPath.join('/')} has invalid bounds 0x${child.toString(16)}–0x${end.toString(16)}.`);
    }
    if (hasSignature(data, child)) validateArchiveStructure(data, child, end, childPath);
  }
}

class CodeReader {
  private position = 0;
  private bitCount = 0;
  private bits = 0;

  constructor(private data: Uint8Array) {}

  read() {
    while (this.bitCount <= 24) {
      if (this.position >= this.data.length) throw new Error('Compressed string ended before its end code.');
      this.bits = (this.bits | (this.data[this.position] << (24 - this.bitCount))) >>> 0;
      this.position += 1;
      this.bitCount += 8;
    }
    const code = this.bits >>> 18;
    this.bits = (this.bits << 14) >>> 0;
    this.bitCount -= 14;
    return code;
  }
}

function decompress(payload: Uint8Array) {
  if (payload.length < 5) throw new Error('Compressed string payload is too small.');
  const expected = u32(payload, 0);
  const padded = new Uint8Array(payload.length);
  padded.set(payload.subarray(4));
  const reader = new CodeReader(padded);
  const prefix = new Uint16Array(0x4000);
  const suffix = new Uint8Array(0x4000);
  let nextCode = 0x100;

  const expand = (initial: number) => {
    let code = initial;
    const reversed: number[] = [];
    while (code > 0xff) {
      if (code >= nextCode) throw new Error(`Invalid dictionary reference 0x${code.toString(16)}.`);
      reversed.push(suffix[code]);
      code = prefix[code];
      if (reversed.length >= 0xfa0) throw new Error('Compressed string dictionary chain is too long.');
    }
    reversed.push(code);
    return reversed.reverse();
  };

  let oldCode = reader.read();
  if (oldCode > 0xff) throw new Error('Compressed string starts with a dictionary code.');
  const output = [oldCode];
  while (true) {
    const code = reader.read();
    if (code === 0x3fff) break;
    let expanded: number[];
    if (code >= nextCode) {
      if (code !== nextCode) throw new Error(`Future dictionary reference 0x${code.toString(16)}.`);
      expanded = expand(oldCode);
      expanded.push(expanded[0]);
    } else {
      expanded = expand(code);
    }
    output.push(...expanded);
    if (nextCode <= 0x3ffe) {
      prefix[nextCode] = oldCode;
      suffix[nextCode] = expanded[0];
      nextCode += 1;
    }
    oldCode = code;
  }
  if (output.length !== expected) {
    throw new Error(`Decoded ${output.length} bytes; record declares ${expected}.`);
  }
  return Uint8Array.from(output);
}

export function extractGameOneArchive(data: Uint8Array): GameOneArchiveEntry[] {
  const nodes = archiveNodes(data);
  const starts = [...new Set([...nodes.map((node) => node.offset), data.length])].sort((a, b) => a - b);
  return nodes.filter((node) => !node.container).map((node) => {
    const end = starts.find((start) => start > node.offset);
    const id = node.path.map((part) => String(part).padStart(3, '0')).join('/');
    if (end === undefined) return { id, path: node.path, packed: new Uint8Array(), error: 'Cannot find entry end.' };
    const packed = data.slice(node.offset, end);
    try {
      return { id, path: node.path, packed, data: decompress(packed) };
    } catch (error) {
      return { id, path: node.path, packed, error: error instanceof Error ? error.message : String(error) };
    }
  });
}

function parseTokens(bytes: Uint8Array) {
  const tokens: GlyphToken[] = [];
  for (let index = 0; index < bytes.length; ) {
    const first = bytes[index];
    if (first === 0) {
      tokens.push({ type: 'end' });
      index += 1;
    } else if (first === 0x5c && index + 1 < bytes.length && (bytes[index + 1] === 0x4e || bytes[index + 1] === 0x6e)) {
      tokens.push({ type: 'newline' });
      index += 2;
    } else if (first & 0x80) {
      if (index + 1 >= bytes.length) throw new Error('String ends with an incomplete two-byte glyph ID.');
      tokens.push({ type: 'glyph', id: ((first & 0x7f) << 8) | bytes[index + 1] });
      index += 2;
    } else {
      tokens.push({ type: 'ascii', byte: first, text: String.fromCharCode(first) });
      index += 1;
    }
  }
  return tokens;
}

export function parseStrings(data: Uint8Array): StringRecord[] {
  const nodes = archiveNodes(data);
  const starts = [...new Set([...nodes.map((node) => node.offset), data.length])].sort((a, b) => a - b);
  return nodes
    .filter((node) => !node.container)
    .map((node) => {
      const end = starts.find((start) => start > node.offset);
      if (end === undefined) throw new Error(`Cannot find the end of string ${node.path.join('/')}.`);
      const bytes = decompress(data.slice(node.offset, end));
      const tokens = parseTokens(bytes);
      if (tokens.at(-1)?.type !== 'end') throw new Error(`String ${node.path.join('/')} is not NUL-terminated.`);
      return {
        id: node.path.map((part) => String(part).padStart(3, '0')).join('/'),
        path: node.path,
        bytes,
        tokens
      };
    });
}

function packCodes(codes: number[]) {
  const output: number[] = [];
  let bits = 0;
  let bitCount = 0;
  for (const code of codes) {
    bits = bits * 0x4000 + code;
    bitCount += 14;
    while (bitCount >= 8) {
      bitCount -= 8;
      output.push(Math.floor(bits / 2 ** bitCount) & 0xff);
      bits %= 2 ** bitCount;
    }
  }
  if (bitCount) output.push((bits * 2 ** (8 - bitCount)) & 0xff);
  return Uint8Array.from(output);
}

function compress(bytes: Uint8Array) {
  if (!bytes.length) throw new Error('Cannot compress an empty string.');
  const dictionary = new Map<string, number>();
  for (let byte = 0; byte < 256; byte += 1) dictionary.set(String.fromCharCode(byte), byte);
  let nextCode = 0x100;
  let phrase = String.fromCharCode(bytes[0]);
  const codes: number[] = [];
  for (const byte of bytes.slice(1)) {
    const character = String.fromCharCode(byte);
    const extended = phrase + character;
    if (dictionary.has(extended)) {
      phrase = extended;
      continue;
    }
    codes.push(dictionary.get(phrase)!);
    if (nextCode <= 0x3ffe) dictionary.set(extended, nextCode++);
    phrase = character;
  }
  codes.push(dictionary.get(phrase)!, 0x3fff);
  const packed = packCodes(codes);
  const payload = new Uint8Array(4 + packed.length);
  putU32(payload, 0, bytes.length);
  payload.set(packed, 4);
  if (!decompress(payload).every((byte, index) => byte === bytes[index])) throw new Error('Internal compression verification failed.');
  return payload;
}

function encodeTranslation(text: string) {
  const normalized = text.replaceAll('\r\n', '\n').replaceAll('\r', '\n').replaceAll('\n', '\\N');
  const bytes = new Uint8Array(normalized.length + 1);
  for (let index = 0; index < normalized.length; index += 1) {
    const code = normalized.charCodeAt(index);
    if (code > 0x7f) throw new Error(`Translation contains non-ASCII character ${JSON.stringify(normalized[index])}.`);
    bytes[index] = code;
  }
  return bytes;
}

function rebuildContainer(original: Uint8Array, offset: number, path: number[], replacements: Map<string, Uint8Array>): Uint8Array {
  if (!hasSignature(original, offset)) throw new Error(`Missing container at 0x${offset.toString(16)}.`);
  const count = u32(original, offset + 0x42);
  const table = offset + 0x66;
  const childOffsets = Array.from({ length: count }, (_, index) => offset + u32(original, table + index * 4));
  const firstChild = childOffsets.length ? Math.min(...childOffsets) : table;
  const header = original.slice(offset, firstChild);
  const children = childOffsets.map((childOffset, index) => {
    const childPath = [...path, index];
    if (hasSignature(original, childOffset)) return rebuildContainer(original, childOffset, childPath, replacements);
    const key = childPath.map((part) => String(part).padStart(3, '0')).join('/');
    const replacement = replacements.get(key);
    if (!replacement) throw new Error(`Missing translation payload ${key}.`);
    return replacement;
  });
  let cursor = header.length;
  for (let index = 0; index < children.length; index += 1) {
    putU32(header, 0x66 + index * 4, cursor);
    cursor += children[index].length;
  }
  // There is one additional offset after the visible child offsets. The game
  // uses it as the end boundary (and therefore the packed size) of the final
  // child. Leaving the original value here corrupts the last record whenever a
  // rebuilt container changes size.
  putU32(header, 0x66 + count * 4, cursor);
  const output = new Uint8Array(cursor);
  output.set(header);
  cursor = header.length;
  for (const child of children) {
    output.set(child, cursor);
    cursor += child.length;
  }
  return output;
}

export function rebuildGameOneArchive(original: Uint8Array, decodedReplacements: ReadonlyMap<string, Uint8Array>) {
  const replacements = new Map<string, Uint8Array>();
  const nodes = archiveNodes(original);
  const starts = [...new Set([...nodes.map((node) => node.offset), original.length])].sort((a, b) => a - b);
  const leafIds = new Set<string>();

  for (const node of nodes.filter((candidate) => !candidate.container)) {
    const end = starts.find((start) => start > node.offset);
    if (end === undefined) throw new Error(`Cannot find original payload end for ${node.path.join('/')}.`);
    const id = node.path.map((part) => String(part).padStart(3, '0')).join('/');
    leafIds.add(id);
    replacements.set(id, original.slice(node.offset, end));
  }

  for (const [id, decoded] of decodedReplacements) {
    if (!leafIds.has(id)) throw new Error(`Archive has no record ${id}.`);
    replacements.set(id, compress(decoded));
  }

  const rebuilt = rebuildContainer(original, 0, [], replacements);
  validateArchiveStructure(rebuilt);
  const verified = new Map(extractGameOneArchive(rebuilt).map((entry) => [entry.id, entry]));
  for (const [id, expected] of decodedReplacements) {
    const actual = verified.get(id);
    if (!actual?.data || actual.data.length !== expected.length || !actual.data.every((byte, index) => byte === expected[index])) {
      throw new Error(`Archive verification failed for replacement ${id}.`);
    }
  }
  return rebuilt;
}

export function rebuildStrings(original: Uint8Array, records: StringRecord[], translations: Record<string, string>) {
  const replacements = new Map<string, Uint8Array>();
  for (const record of records) {
    const translation = translations[record.id];
    if (translation !== undefined && translation.length > 0) replacements.set(record.id, encodeTranslation(translation));
  }
  const rebuilt = rebuildGameOneArchive(original, replacements);
  const verified = parseStrings(rebuilt);
  if (verified.length !== records.length) throw new Error(`Rebuilt archive has ${verified.length} records instead of ${records.length}.`);
  const verifiedById = new Map(verified.map((record) => [record.id, record.bytes]));
  for (const record of records) {
    const translation = translations[record.id];
    if (translation === undefined || translation.length === 0) continue;
    const expected = encodeTranslation(translation);
    const actual = verifiedById.get(record.id);
    if (!actual || actual.length !== expected.length || !actual.every((byte, index) => byte === expected[index])) {
      throw new Error(`Export verification failed for translated record ${record.id}.`);
    }
  }
  return rebuilt;
}

export function parseSysFont(data: Uint8Array): SysFont {
  if (data.length < 6) throw new Error('sysfont.dat is too small.');
  const count = u16(data, 0);
  if (!count || count % 128 !== 0 || 2 + count * 4 > data.length) {
    throw new Error(`Invalid sysfont glyph count ${count}.`);
  }
  const glyphs: SysGlyph[] = [];
  for (let index = 0; index < count; index += 1) {
    const offset = u32(data, 2 + index * 4);
    if (offset + 2 > data.length) throw new Error(`sysfont glyph ${index} points outside the file.`);
    const width = data[offset];
    const height = data[offset + 1];
    const end = offset + 2 + width * height;
    if (!width || !height || width > 96 || height > 96 || end > data.length) {
      throw new Error(`sysfont glyph ${index} has invalid dimensions ${width}×${height}.`);
    }
    glyphs.push({ width, height, pixels: data.slice(offset + 2, end) });
  }
  return { bytes: data, count, variants: count / 128, glyphs };
}

export function rebuildSysFont(font: SysFont) {
  if (font.glyphs.length !== font.count) throw new Error(`Expected ${font.count} sysfont glyphs; got ${font.glyphs.length}.`);
  const tableEnd = 2 + font.count * 4;
  const firstGlyphOffset = u32(font.bytes, 2);
  if (firstGlyphOffset < tableEnd || firstGlyphOffset > font.bytes.length) throw new Error('Invalid sysfont first glyph offset.');
  let length = firstGlyphOffset;
  for (const glyph of font.glyphs) {
    if (!glyph.width || !glyph.height || glyph.width > 96 || glyph.height > 96 || glyph.pixels.length !== glyph.width * glyph.height) {
      throw new Error(`Invalid sysfont glyph ${glyph.width}×${glyph.height}.`);
    }
    length += 2 + glyph.pixels.length;
  }
  const output = new Uint8Array(length);
  output.set(font.bytes.slice(0, firstGlyphOffset));
  let cursor = firstGlyphOffset;
  for (let index = 0; index < font.glyphs.length; index += 1) {
    const glyph = font.glyphs[index];
    putU32(output, 2 + index * 4, cursor);
    output[cursor] = glyph.width;
    output[cursor + 1] = glyph.height;
    output.set(glyph.pixels, cursor + 2);
    cursor += 2 + glyph.pixels.length;
  }
  const verified = parseSysFont(output);
  if (verified.count !== font.count || verified.glyphs.some((glyph, index) => glyph.width !== font.glyphs[index].width || glyph.height !== font.glyphs[index].height || !glyph.pixels.every((value, pixel) => value === font.glyphs[index].pixels[pixel]))) {
    throw new Error('Rebuilt sysfont verification failed.');
  }
  return output;
}

export function validateChiFont(data: Uint8Array) {
  if (!data.length || data.length % 32 !== 0) {
    throw new Error(`chifont.dat must be a non-empty multiple of 32 bytes; got ${data.length}.`);
  }
  return data.length / 32;
}

export function tokenLabel(token: GlyphToken) {
  if (token.type === 'glyph') return `g${token.id}`;
  if (token.type === 'end') return 'NUL';
  if (token.type === 'newline') return '↵';
  const printable = token.byte >= 32 && token.byte < 127 ? JSON.stringify(token.text) : '';
  return `${printable || 'byte'}[${token.byte.toString(16).padStart(2, '0')}]`;
}
