import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { patchVietnameseExecutable } from './patch-vietnamese-exe';

describe('Vietnamese executable patch', () => {
  test('patches only the verified executable and exposes the CSEG cave', () => {
    const original = new Uint8Array(readFileSync(new URL('../../tmp/Doraemon.exe', import.meta.url)));
    const { output, caveBytes } = patchVietnameseExecutable(original);
    expect(output).toHaveLength(original.length);
    expect(caveBytes).toBeGreaterThan(0);
    expect(caveBytes).toBeLessThanOrEqual(1024);
    for (const raw of [0xcc1d0, 0xcc235, 0xcc2e1, 0xcc444]) expect(output[raw]).toBe(0xe9);
    expect(output.slice(0xcdc00).every((byte, index) => byte === original[index])).toBe(false);
    expect(output[0xcdc00]).toBe(0x3c);
    expect(new TextDecoder().decode(output.slice(0xcb00a, 0xcb019)).replaceAll('\0', '')).toBe(
      'sysfont-vi.dat'
    );
  });

  test('rejects an unknown executable', () => {
    expect(() => patchVietnameseExecutable(new Uint8Array(128))).toThrow('Unsupported Doraemon.exe');
  });
});
