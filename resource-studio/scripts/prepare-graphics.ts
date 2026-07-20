import { createHash } from 'node:crypto';
import { mkdir, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';
import {
  parsePcx,
  readBitmaps,
  readMapElements,
  readSprites,
  type IndexedImage
} from '../src/lib/asset-formats';
import { extractGameOneArchive } from '../src/lib/formats';
import { parseMapAnimations, parseMapLayout } from '../src/lib/map-formats';
import { decodeVoiceRecord, parseVoiceArchive, parseWav } from '../src/lib/voice-formats';

const pageSize = 96;
const studio = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const game = resolve(studio, 'public', 'game');
const prepared = resolve(game, 'prepared');

function mapIds(available: Set<string>) {
  return [...available]
    .map((file) => /^map(\d{4})\.dat$/i.exec(file)?.[1])
    .filter((suffix): suffix is string => Boolean(suffix))
    .filter((suffix) => available.has(`mapElem${suffix}.dat`))
    .map(Number)
    .sort((left, right) => left - right);
}

type PreparedImage = Omit<IndexedImage, 'pixels' | 'alpha' | 'palette' | 'pcxHeader'> & {
  pixels: string;
  alpha?: string;
  palette?: string;
  pcxHeader?: string;
};

type CatalogueEntry = Pick<IndexedImage, 'id' | 'width' | 'height' | 'kind'> & { page: number };

type PreparedArchive = {
  name: string;
  count: number;
  pages: number;
  entries: CatalogueEntry[];
};

type PreparedMap = {
  id: number;
  name: string;
  width: number;
  height: number;
  layoutUrl: string;
  elements: PreparedArchive;
  palette?: string;
  preview?: PreparedImage;
  objectCount: number;
  animationCount: number;
};

type PreparedVoiceRecord = {
  id: string;
  path: [number, number, number];
  storage: 'raw' | 'compressed' | 'empty';
  url?: string;
  duration?: number;
  sampleRate?: number;
  bitsPerSample?: number;
  hash?: string;
};

const pack = (bytes: Uint8Array | undefined) => (bytes ? Buffer.from(bytes).toString('base64') : undefined);

function serialise(image: IndexedImage): PreparedImage {
  return {
    ...image,
    pixels: pack(image.pixels)!,
    alpha: pack(image.alpha),
    palette: pack(image.palette),
    pcxHeader: pack(image.pcxHeader)
  };
}

async function prepare(name: string, loader: (bytes: Uint8Array) => { images: IndexedImage[] }) {
  const bytes = new Uint8Array(await readFile(resolve(game, name)));
  const images = loader(bytes).images;
  const directory = resolve(prepared, name);
  await mkdir(directory, { recursive: true });

  for (let start = 0; start < images.length; start += pageSize) {
    const page = Math.floor(start / pageSize);
    // Keep the gzip payload under a neutral extension. Vite and some proxies
    // automatically decompress .gz files, which corrupts a client-side second
    // decompression attempt.
    await writeFile(
      resolve(directory, `page-${page}.prepared`),
      gzipSync(JSON.stringify({ version: 1, images: images.slice(start, start + pageSize).map(serialise) }))
    );
  }

  const archive: PreparedArchive = {
    name,
    count: images.length,
    pages: Math.ceil(images.length / pageSize),
    entries: images.map((image, index) => ({
      id: image.id,
      width: image.width,
      height: image.height,
      kind: image.kind,
      page: Math.floor(index / pageSize)
    }))
  };
  console.log(
    `Prepared ${images.length.toLocaleString()} decoded records in ${archive.pages} pages for ${name}.`
  );
  return archive;
}

function sourceName(bytes: Uint8Array, fallback: string) {
  const text = new TextDecoder('latin1').decode(bytes);
  const match = /(?:Map\\|Map\/)([^\\/]+)\\(?:Map_Block_[^\\]+|All map)\.bmp/i.exec(text);
  return match?.[1]?.replace(/_/g, ' ') ?? fallback;
}

async function prepareMap(id: number): Promise<PreparedMap> {
  const suffix = String(id).padStart(4, '0');
  const mapName = `map${suffix}.dat`;
  const elementsName = `mapElem${suffix}.dat`;
  const mapBytes = new Uint8Array(await readFile(resolve(game, mapName)));
  const result = readMapElements(new Uint8Array(await readFile(resolve(game, elementsName))));
  const directory = resolve(prepared, elementsName);
  await mkdir(directory, { recursive: true });
  for (let start = 0; start < result.images.length; start += pageSize) {
    const page = Math.floor(start / pageSize);
    await writeFile(
      resolve(directory, `page-${page}.prepared`),
      gzipSync(
        JSON.stringify({ version: 1, images: result.images.slice(start, start + pageSize).map(serialise) })
      )
    );
  }
  const mapEntries = extractGameOneArchive(mapBytes);
  const layout = parseMapLayout(mapBytes);
  const animations = parseMapAnimations(result.entries);
  const layoutDirectory = resolve(prepared, mapName);
  await mkdir(layoutDirectory, { recursive: true });
  await writeFile(
    resolve(layoutDirectory, 'layout.prepared'),
    gzipSync(JSON.stringify({ version: 1, layout, animations }))
  );
  const previewEntry = mapEntries.find((entry) => entry.id === '001');
  const preview = previewEntry ? parsePcx({ ...previewEntry, data: previewEntry.packed }) : undefined;
  return {
    id,
    name: sourceName(mapBytes, `Map ${id}`),
    width: layout.width,
    height: layout.height,
    layoutUrl: `/game/prepared/${mapName}/layout.prepared`,
    elements: {
      name: elementsName,
      count: result.images.length,
      pages: Math.ceil(result.images.length / pageSize),
      entries: result.images.map((image, index) => ({
        id: image.id,
        width: image.width,
        height: image.height,
        kind: image.kind,
        page: Math.floor(index / pageSize)
      }))
    },
    palette: pack(result.palette),
    preview: preview ? serialise(preview) : undefined,
    objectCount: layout.objects.length,
    animationCount: animations.length
  };
}

async function exists(path: string) {
  try {
    await stat(path);
    return true;
  } catch {
    return false;
  }
}

async function prepareVoice(name: string, audioDirectory: string) {
  const archive = parseVoiceArchive(new Uint8Array(await readFile(resolve(game, name))));
  const records: PreparedVoiceRecord[] = [];
  for (const record of archive.records) {
    const wav = decodeVoiceRecord(archive, record);
    if (!wav) {
      records.push({ id: record.id, path: record.path, storage: record.storage });
      continue;
    }
    const hash = createHash('sha256').update(wav).digest('hex');
    const filename = `${hash}.wav`;
    const destination = resolve(audioDirectory, filename);
    if (!(await exists(destination))) await writeFile(destination, wav);
    const info = parseWav(wav);
    records.push({
      id: record.id,
      path: record.path,
      storage: record.storage,
      url: `/game/prepared/voice/audio/${filename}`,
      duration: info.duration,
      sampleRate: info.sampleRate,
      bitsPerSample: info.bitsPerSample,
      hash
    });
  }
  return { name, characters: archive.characters, bankCounts: archive.bankCounts, records };
}

await rm(prepared, { recursive: true, force: true });
await mkdir(prepared, { recursive: true });
const archives = await Promise.all([
  prepare('bitmaps.dat', readBitmaps),
  prepare('Sprite1.dat', readSprites),
  prepare('sprite2.dat', readSprites)
]);
await writeFile(resolve(prepared, 'catalogue.json'), JSON.stringify({ version: 1, pageSize, archives }));
const maps = await Promise.all(mapIds(new Set(await readdir(game))).map(prepareMap));
await writeFile(resolve(prepared, 'maps.json'), JSON.stringify({ version: 1, pageSize, maps }));

const originalVoice = resolve(game, 'voice-origin.dat');
const workingVoice = resolve(game, 'voice.dat');
if ((await exists(originalVoice)) && (await exists(workingVoice))) {
  const audioDirectory = resolve(prepared, 'voice', 'audio');
  await mkdir(audioDirectory, { recursive: true });
  // Process these sequentially. Each archive is large, so retaining both source
  // buffers during extraction unnecessarily doubles peak preparation memory.
  const original = await prepareVoice('voice-origin.dat', audioDirectory);
  const working = await prepareVoice('voice.dat', audioDirectory);
  await writeFile(
    resolve(prepared, 'voice', 'manifest.json'),
    JSON.stringify({
      version: 1,
      characters: ['Doraemon', 'Nobita', 'Dorami', 'Shizuka', 'Suneo', 'Gian'],
      original,
      working
    })
  );
  console.log(`Prepared ${working.records.length.toLocaleString()} voice slots for voice.dat.`);
}
