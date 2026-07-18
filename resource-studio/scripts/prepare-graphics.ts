import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
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
