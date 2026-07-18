import { expect, test } from 'bun:test';
import { parseMapAnimation, parseMapLayoutRecords } from './map-formats';
import type { GameOneArchiveEntry } from './formats';

function put32(data: Uint8Array, offset: number, value: number) {
  new DataView(data.buffer).setUint32(offset, value >>> 0, true);
}

test('decodes cells, object names, starts, directions, and special locations', () => {
  const paths = new TextEncoder().encode(
    'K:\\Map\\Map_Block_Test.bmp\0K:\\Map\\common\\MB1.bmp\0K:\\Map\\Tree.bmp\0'
  );
  const grid = new Uint8Array(80 + 2 * 16 + paths.length);
  put32(grid, 4, 2);
  put32(grid, 8, 1);
  put32(grid, 48, 7);
  put32(grid, 80, 0x00000000);
  put32(grid, 84, 0x000300c7);
  put32(grid, 88, 19);
  put32(grid, 92, 4);
  put32(grid, 96, 0xffff0000);
  put32(grid, 100, 0x000500c0);
  grid.set(paths, 112);

  const starts = new Uint8Array(280);
  put32(starts, 0, 6);
  for (let player = 0; player < 4; player += 1) {
    put32(starts, 4 + player * 12, 10 + player);
    put32(starts, 8 + player * 12, 20 + player);
    put32(starts, 12 + player * 12, player);
    put32(starts, 52 + player * 4, 3 - player);
  }
  put32(starts, 0x44, 18);
  put32(starts, 0x48, 18);
  put32(starts, 0x4c, 3);

  const special = new Uint8Array(4 + 348);
  put32(special, 0, 1);
  put32(special, 4, 30);
  put32(special, 8, 31);
  put32(special, 12, 2);
  put32(special, 16, 100);

  const layout = parseMapLayoutRecords(grid, starts, special);
  expect(layout).toMatchObject({ mapId: 6, width: 2, height: 1, terrainCount: 7 });
  expect(layout.cells[0]).toMatchObject({
    x: 0,
    y: 0,
    objectId: 0,
    terrainId: 3,
    terrainFlags: 0xc7,
    routeValue: 19,
    regionId: 4
  });
  expect(layout.objects[0]).toMatchObject({
    assetId: '002/000',
    name: 'MB1.bmp',
    family: 'MB',
    common: true
  });
  expect(layout.objects[0].placements).toEqual([{ x: 0, y: 0, cellIndex: 0 }]);
  expect(layout.starts[3]).toMatchObject({ x: 13, y: 23, z: 3, directionCode: 0 });
  expect(layout.jail).toEqual({
    offset: 0x44,
    x: 18,
    y: 18,
    directionCode: 3,
    bombX: 14,
    bombY: 18,
    approachCells: [
      { x: 15, y: 18 },
      { x: 16, y: 18 },
      { x: 17, y: 18 }
    ]
  });
  expect(layout.specialLocations[0]).toMatchObject({ x: 30, y: 31, z: 2 });
  expect(layout.specialLocations[0].parameterWords).toHaveLength(84);
  expect(layout.specialLocations[0].parameterWords[0]).toBe(100);
});

test('decodes group-004 frame references while preserving variable frame blocks', () => {
  const data = new Uint8Array(100);
  put32(data, 0, 36);
  put32(data, 4, 4);
  put32(data, 8, -2);
  put32(data, 12, 3);
  put32(data, 16, 4);
  for (let index = 0; index < 4; index += 1) {
    put32(data, 20 + index * 4, 29 + index);
    data.fill(index + 1, 36 + index * 16, 52 + index * 16);
  }
  const entry: GameOneArchiveEntry = { id: '004/000', path: [4, 0], packed: data, data };
  const animation = parseMapAnimation(entry)!;
  expect(animation).toMatchObject({ width: 36, height: 4, originX: -2, originY: 3 });
  expect(animation.frameIds).toEqual(['003/029', '003/030', '003/031', '003/032']);
  expect(animation.frames).toHaveLength(4);
  expect(animation.frames[0].rawBytes).toHaveLength(16);
  expect(animation.frames[3].rawBytes.every((byte) => byte === 4)).toBe(true);
});
