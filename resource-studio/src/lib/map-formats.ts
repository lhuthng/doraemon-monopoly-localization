import { extractGameOneArchive, type GameOneArchiveEntry } from './formats';

export type MapCell = {
  index: number;
  offset: number;
  x: number;
  y: number;
  objectId?: number;
  objectFlags: number;
  terrainId?: number;
  terrainFlags: number;
  routeValue: number;
  regionId: number;
  rawWords: number[];
};

export type MapObject = {
  id: number;
  assetId: string;
  sourcePath?: string;
  name: string;
  family: string;
  common: boolean;
  placements: { x: number; y: number; cellIndex: number }[];
};

export type MapStart = {
  player: number;
  offset: number;
  x: number;
  y: number;
  z: number;
  directionCode: number;
};

export type MapSpecialLocation = {
  index: number;
  offset: number;
  x: number;
  y: number;
  z: number;
  parameterWords: number[];
  rawWords: number[];
};

export type MapJailConfiguration = {
  offset: number;
  x: number;
  y: number;
  directionCode: number;
  bombX: number;
  bombY: number;
  approachCells: { x: number; y: number }[];
};

export type MapAnimationFrame = {
  index: number;
  assetId: string;
  sourceOffset: number;
  rawBytes: number[];
  rawWords: number[];
};

export type MapAnimationDefinition = {
  id: string;
  width: number;
  height: number;
  originX: number;
  originY: number;
  frameIds: string[];
  controlBytes: number[];
  controlWords: number[];
  frames: MapAnimationFrame[];
  footerBytes: number[];
  rawBytes: number;
};

export type MapLayout = {
  mapId: number;
  width: number;
  height: number;
  terrainCount: number;
  headerWords: number[];
  cells: MapCell[];
  previewSourcePath?: string;
  objects: MapObject[];
  starts: MapStart[];
  jail: MapJailConfiguration;
  startRecordWords: number[];
  specialLocations: MapSpecialLocation[];
};

const eventClassNames: Record<number, string> = {
  0: 'None',
  1: 'Purchasable land',
  4: 'Mini-game',
  5: 'Shop',
  6: 'Bank',
  7: 'Extra turn',
  8: 'Bomb',
  9: 'Hole',
  10: 'Bonus',
  11: 'Penalty',
  12: 'Penalty-pool payout',
  13: 'Big event (unverified)',
  14: 'Hermit event',
  15: 'Animation trigger'
};

export function eventClassName(id: number) {
  return eventClassNames[id] ?? 'Unknown event class';
}

const u32 = (data: Uint8Array, offset: number) =>
  (data[offset] | (data[offset + 1] << 8) | (data[offset + 2] << 16) | (data[offset + 3] << 24)) >>> 0;
const i32 = (data: Uint8Array, offset: number) => u32(data, offset) | 0;
const words = (data: Uint8Array, offset = 0, length = data.length - offset) =>
  Array.from({ length: Math.floor(length / 4) }, (_, index) => u32(data, offset + index * 4));

function requireRecord(entries: GameOneArchiveEntry[], id: string) {
  const entry = entries.find((candidate) => candidate.id === id);
  if (!entry) throw new Error(`Map archive is missing record ${id}.`);
  return entry.packed;
}

function bitmapPaths(data: Uint8Array) {
  const text = new TextDecoder('latin1').decode(data);
  return [...text.matchAll(/[A-Z]:\\[^\0]+?\.bmp/gi)].map((match) => match[0]);
}

function basename(path: string | undefined, fallback: string) {
  return path?.split('\\').at(-1) ?? fallback;
}

function family(name: string) {
  const stem = name.replace(/\.bmp$/i, '');
  return /^M[BGYR]/i.test(stem) ? stem.slice(0, 2).toUpperCase() : (/^[A-Za-z]+/.exec(stem)?.[0] ?? stem);
}

export function parseMapLayout(data: Uint8Array): MapLayout {
  const entries = extractGameOneArchive(data);
  return parseMapLayoutRecords(
    requireRecord(entries, '000'),
    requireRecord(entries, '002'),
    requireRecord(entries, '003')
  );
}

export function parseMapLayoutRecords(
  grid: Uint8Array,
  startsRecord: Uint8Array,
  specialRecord: Uint8Array
): MapLayout {
  if (grid.length < 80) throw new Error('Map grid record is shorter than its 80-byte header.');
  const width = u32(grid, 4);
  const height = u32(grid, 8);
  if (!width || !height || width > 512 || height > 512)
    throw new Error(`Invalid map dimensions ${width}×${height}.`);
  const cellsEnd = 80 + width * height * 16;
  if (cellsEnd > grid.length) throw new Error('Map cell grid extends beyond record 000.');
  const cells: MapCell[] = [];
  for (let index = 0; index < width * height; index += 1) {
    const offset = 80 + index * 16;
    const rawWords = words(grid, offset, 16);
    const objectRaw = rawWords[0];
    const terrainRaw = rawWords[1];
    const objectId = objectRaw >>> 16;
    const terrainId = terrainRaw >>> 16;
    cells.push({
      index,
      offset,
      x: index % width,
      y: Math.floor(index / width),
      objectId: objectId === 0xffff ? undefined : objectId,
      objectFlags: objectRaw & 0xffff,
      terrainId: terrainId === 0xffff ? undefined : terrainId,
      terrainFlags: terrainRaw & 0xffff,
      routeValue: rawWords[2],
      regionId: rawWords[3],
      rawWords
    });
  }

  const paths = bitmapPaths(grid.subarray(cellsEnd));
  const objectCount = Math.max(0, paths.length - 1);
  const objects = Array.from({ length: objectCount }, (_, id): MapObject => {
    const sourcePath = paths[id + 1];
    const name = basename(sourcePath, `Object ${id}`);
    return {
      id,
      assetId: `002/${String(id).padStart(3, '0')}`,
      sourcePath,
      name,
      family: family(name),
      common: id < 298,
      placements: cells
        .filter((cell) => cell.objectId === id)
        .map((cell) => ({ x: cell.x, y: cell.y, cellIndex: cell.index }))
    };
  });

  if (startsRecord.length < 80) throw new Error('Map start/configuration record is too short.');
  const starts = Array.from({ length: 4 }, (_, player): MapStart => ({
    player,
    offset: 4 + player * 12,
    x: i32(startsRecord, 4 + player * 12),
    y: i32(startsRecord, 8 + player * 12),
    z: i32(startsRecord, 12 + player * 12),
    directionCode: u32(startsRecord, 52 + player * 4)
  }));
  const jailX = i32(startsRecord, 0x44);
  const jailY = i32(startsRecord, 0x48);
  const jailDirection = u32(startsRecord, 0x4c);
  const jailVector = jailDirection === 3 ? { x: 1, y: 0 } : { x: 0, y: 1 };
  const jail: MapJailConfiguration = {
    offset: 0x44,
    x: jailX,
    y: jailY,
    directionCode: jailDirection,
    bombX: jailX - jailVector.x * 4,
    bombY: jailY - jailVector.y * 4,
    approachCells: [3, 2, 1].map((distance) => ({
      x: jailX - jailVector.x * distance,
      y: jailY - jailVector.y * distance
    }))
  };

  if (specialRecord.length < 4) throw new Error('Map special-location record is too short.');
  const specialCount = u32(specialRecord, 0);
  const specialStride = 348;
  if (4 + specialCount * specialStride > specialRecord.length)
    throw new Error('Map special-location records extend beyond record 003.');
  const specialLocations = Array.from({ length: specialCount }, (_, index): MapSpecialLocation => {
    const offset = 4 + index * specialStride;
    const rawWords = words(specialRecord, offset, specialStride);
    return {
      index,
      offset,
      x: rawWords[0] | 0,
      y: rawWords[1] | 0,
      z: rawWords[2] | 0,
      parameterWords: rawWords.slice(3),
      rawWords
    };
  });

  return {
    mapId: u32(startsRecord, 0),
    width,
    height,
    terrainCount: u32(grid, 48),
    headerWords: words(grid, 0, 80),
    cells,
    previewSourcePath: paths[0],
    objects,
    starts,
    jail,
    startRecordWords: words(startsRecord),
    specialLocations
  };
}

export function parseMapAnimation(entry: GameOneArchiveEntry): MapAnimationDefinition | undefined {
  const data = entry.data;
  if (!data || entry.path[0] !== 4 || data.length < 20) return;
  const frameCount = u32(data, 16);
  const referencesEnd = 20 + frameCount * 4;
  if (frameCount > 10_000 || referencesEnd > data.length) return;
  const referenceIds = Array.from({ length: frameCount }, (_, index) => u32(data, 20 + index * 4));
  const remaining = data.length - referencesEnd;
  const frameStride = frameCount ? Math.floor(remaining / frameCount) : 0;
  const controlLength = frameCount ? remaining - frameStride * frameCount : remaining;
  const controlBytes = [...data.slice(referencesEnd, referencesEnd + controlLength)];
  const controlWords = words(data, referencesEnd, controlLength);
  const framesStart = referencesEnd + controlLength;
  const availableFrames = frameStride
    ? Math.min(frameCount, Math.floor((data.length - framesStart) / frameStride))
    : 0;
  const frames = Array.from({ length: availableFrames }, (_, index): MapAnimationFrame => ({
    index,
    assetId: `003/${String(referenceIds[index]).padStart(3, '0')}`,
    sourceOffset: framesStart + index * frameStride,
    rawBytes: [...data.slice(framesStart + index * frameStride, framesStart + (index + 1) * frameStride)],
    rawWords: words(data, framesStart + index * frameStride, frameStride)
  }));
  const footerStart = framesStart + availableFrames * frameStride;
  return {
    id: entry.id,
    width: u32(data, 0),
    height: u32(data, 4),
    originX: i32(data, 8),
    originY: i32(data, 12),
    frameIds: referenceIds.map((id) => `003/${String(id).padStart(3, '0')}`),
    controlBytes,
    controlWords,
    frames,
    footerBytes: [...data.slice(footerStart)],
    rawBytes: data.length
  };
}

export function parseMapAnimations(entries: GameOneArchiveEntry[]) {
  return entries.map(parseMapAnimation).filter((entry): entry is MapAnimationDefinition => Boolean(entry));
}
