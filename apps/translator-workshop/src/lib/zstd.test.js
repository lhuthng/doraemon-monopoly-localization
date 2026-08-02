import { describe, expect, test } from 'bun:test';
import { readStoredZip, storedZip } from '@doraemon-monopoly/dubbing-core';
import { compressWork, decompressWork } from './zstd.js';

describe('zstd wrapper', () => {
  test('round-trips a byte payload', async () => {
    const payload = new TextEncoder().encode('hello world '.repeat(1000));
    const compressed = await compressWork(payload);
    expect(compressed.length).toBeLessThan(payload.length);
    const restored = await decompressWork(compressed);
    expect(restored).toEqual(payload);
  });
});

describe('zstd work-zip round-trip', () => {
  test('a stored ZIP survives zstd compress + decompress', async () => {
    const entries = [
      { name: 'dubbing/english/manifest.json', bytes: new TextEncoder().encode('{"format":"x"}') },
      { name: 'dubbing/english/voices/doraemon/000-000-001.wav', bytes: new Uint8Array([1, 2, 3, 4]) }
    ];
    const zipBytes = new Uint8Array(await storedZip(entries).arrayBuffer());
    const compressed = await compressWork(zipBytes);
    expect(compressed.length).toBeLessThan(zipBytes.length);
    const restored = await decompressWork(compressed);
    expect(restored).toEqual(zipBytes);
    const parsed = readStoredZip(restored);
    expect(parsed).toHaveLength(2);
    expect(parsed[1].name).toBe('dubbing/english/voices/doraemon/000-000-001.wav');
  });
});
