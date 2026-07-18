import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';
import { readBitmaps, readSprites, type IndexedImage } from '../src/lib/asset-formats';

const pageSize = 96;
const studio = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const game = resolve(studio, 'public', 'game');
const prepared = resolve(game, 'prepared');

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

await rm(prepared, { recursive: true, force: true });
await mkdir(prepared, { recursive: true });
const archives = await Promise.all([
  prepare('bitmaps.dat', readBitmaps),
  prepare('Sprite1.dat', readSprites),
  prepare('sprite2.dat', readSprites)
]);
await writeFile(resolve(prepared, 'catalogue.json'), JSON.stringify({ version: 1, pageSize, archives }));
