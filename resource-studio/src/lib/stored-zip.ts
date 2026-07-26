const encoder = new TextEncoder();
const u16 = (value: number) => Uint8Array.of(value & 255, (value >>> 8) & 255);
const u32 = (value: number) =>
  Uint8Array.of(value & 255, (value >>> 8) & 255, (value >>> 16) & 255, (value >>> 24) & 255);

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
  for (const part of parts) {
    output.set(part, offset);
    offset += part.length;
  }
  return output;
}

export function storedZip(entries: { name: string; bytes: Uint8Array }[]) {
  if (entries.length > 0xffff) throw new Error('ZIP contains too many files.');
  const locals: Uint8Array[] = [];
  const central: Uint8Array[] = [];
  let offset = 0;
  for (const entry of entries) {
    const name = encoder.encode(entry.name);
    if (name.length > 0xffff) throw new Error(`ZIP filename is too long: ${entry.name}.`);
    const crc = crc32(entry.bytes);
    const local = join([
      u32(0x04034b50),
      u16(20),
      u16(0x0800),
      u16(0),
      u16(0),
      u16(0),
      u32(crc),
      u32(entry.bytes.length),
      u32(entry.bytes.length),
      u16(name.length),
      u16(0),
      name,
      entry.bytes
    ]);
    locals.push(local);
    central.push(
      join([
        u32(0x02014b50),
        u16(20),
        u16(20),
        u16(0x0800),
        u16(0),
        u16(0),
        u16(0),
        u32(crc),
        u32(entry.bytes.length),
        u32(entry.bytes.length),
        u16(name.length),
        u16(0),
        u16(0),
        u16(0),
        u16(0),
        u32(0),
        u32(offset),
        name
      ])
    );
    offset += local.length;
  }
  const directory = join(central);
  const ending = join([
    u32(0x06054b50),
    u16(0),
    u16(0),
    u16(entries.length),
    u16(entries.length),
    u32(directory.length),
    u32(offset),
    u16(0)
  ]);
  return new Blob([join([...locals, directory, ending])], { type: 'application/zip' });
}

function readU16(bytes: Uint8Array, offset: number) {
  return bytes[offset] | (bytes[offset + 1] << 8);
}

function readU32(bytes: Uint8Array, offset: number) {
  return (
    (bytes[offset] | (bytes[offset + 1] << 8) | (bytes[offset + 2] << 16) | (bytes[offset + 3] << 24)) >>> 0
  );
}

/** Reads ZIPs written by storedZip. Compressed ZIPs are intentionally rejected. */
export function readStoredZip(bytes: Uint8Array) {
  const entries: { name: string; bytes: Uint8Array }[] = [];
  let offset = 0;
  while (offset + 4 <= bytes.length && readU32(bytes, offset) === 0x04034b50) {
    if (offset + 30 > bytes.length) throw new Error('Truncated ZIP local header.');
    const flags = readU16(bytes, offset + 6);
    const compression = readU16(bytes, offset + 8);
    const length = readU32(bytes, offset + 18);
    const nameLength = readU16(bytes, offset + 26);
    const extraLength = readU16(bytes, offset + 28);
    if (flags & 8 || compression !== 0) throw new Error('Only uncompressed contributor ZIPs are supported.');
    const start = offset + 30 + nameLength + extraLength;
    const end = start + length;
    if (end > bytes.length) throw new Error('ZIP entry extends beyond the file.');
    const name = new TextDecoder().decode(bytes.subarray(offset + 30, offset + 30 + nameLength));
    entries.push({ name, bytes: bytes.slice(start, end) });
    offset = end;
  }
  if (!entries.length) throw new Error('ZIP has no readable contributor files.');
  return entries;
}
