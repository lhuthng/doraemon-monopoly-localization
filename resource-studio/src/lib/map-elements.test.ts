import { expect, test } from 'bun:test';
import { readMapElementEntries } from './asset-formats';
import type { GameOneArchiveEntry } from './formats';

function sprite(pixel: number) {
  return Uint8Array.of(
    0x02,
    0x80,
    0x01,
    0x00,
    0x01,
    0x00, // 0x8002, 1×1
    0x02,
    0x00, // first row starts two bytes after its fixed header
    0x03,
    0x00,
    0x01,
    0x00,
    pixel // payload length, one literal pixel
  );
}

test('classifies map tiles, RLE sprites, palette, and metadata', () => {
  const palette = new Uint8Array(768);
  palette[1] = 63;
  const entries: GameOneArchiveEntry[] = [
    { id: '000/000', path: [0, 0], packed: sprite(0xff), data: sprite(0xff) },
    {
      id: '001/000',
      path: [1, 0],
      packed: Uint8Array.of(2, 0, 1, 0, 4, 5),
      data: Uint8Array.of(2, 0, 1, 0, 4, 5)
    },
    { id: '002/000', path: [2, 0], packed: sprite(0x2a), data: sprite(0x2a) },
    { id: '004/000', path: [4, 0], packed: Uint8Array.of(1, 2, 3), data: Uint8Array.of(1, 2, 3) },
    { id: '005', path: [5], packed: palette }
  ];
  const result = readMapElementEntries(entries);
  expect(result.palette).toHaveLength(768);
  expect(result.palette![0]).toBe(0);
  expect(result.palette![1]).toBe(255);
  expect(result.images).toHaveLength(3);
  expect(result.images[0]).toMatchObject({ id: '000/000', width: 1, height: 1 });
  expect(result.images[0].pixels).toEqual(Uint8Array.of(0xff));
  expect(result.images[0].alpha).toEqual(Uint8Array.of(0));
  expect(result.images[1]).toMatchObject({ id: '001/000', width: 2, height: 1 });
  expect(result.images[1].alpha).toEqual(Uint8Array.of(255, 255));
  expect(result.images[2]).toMatchObject({ id: '002/000', width: 1, height: 1 });
  expect(result.images[2].alpha).toEqual(Uint8Array.of(255));
  expect(result.metadata).toHaveLength(1);
  expect(result.metadata[0]).toMatchObject({ id: '004/000', group: '4' });
});
