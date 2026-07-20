<script lang="ts">
  import { onMount } from 'svelte';
  import '../../styles/graphics.css';
  import {
    diagnosticPalette,
    encodePcx,
    encodeSprite,
    readBitmaps,
    readSprites,
    type IndexedImage,
    type Palette
  } from '../../lib/asset-formats';
  import { binaryBlob, downloadBlob } from '../../lib/browser-download';
  import { extractGameOneArchive, rebuildGameOneArchive, type GameOneArchiveEntry } from '../../lib/formats';
  import {
    decodeIndexedPng,
    encodeIndexedPng,
    transparencyIndex,
    type IndexedPng
  } from '../../lib/indexed-png';
  import { storedZip } from '../../lib/stored-zip';
  import IndexedCanvas from './components/IndexedCanvas.svelte';
  import AssetTile from './components/AssetTile.svelte';
  import MapStudio from './MapStudio.svelte';

  const pageSize = 96;
  type AssetTab = 'bitmap' | 'sprite1' | 'sprite2';
  type PreparedImage = Omit<IndexedImage, 'pixels' | 'alpha' | 'palette' | 'pcxHeader'> & {
    pixels: string;
    alpha?: string;
    palette?: string;
    pcxHeader?: string;
  };
  type CatalogueEntry = Pick<IndexedImage, 'id' | 'width' | 'height' | 'kind'> & { page: number };
  type PreparedCatalogue = {
    version: number;
    pageSize: number;
    archives: { name: string; count: number; pages: number; entries: CatalogueEntry[] }[];
  };
  let bitmaps: IndexedImage[] = $state([]);
  let sprites: IndexedImage[] = $state([]);
  let sprites2: IndexedImage[] = $state([]);
  let bitmapCatalogue = $state<CatalogueEntry[]>([]);
  let spriteCatalogue = $state<CatalogueEntry[]>([]);
  let sprite2Catalogue = $state<CatalogueEntry[]>([]);
  let bitmapLoaded = $state(new Map<string, IndexedImage>());
  let spriteLoaded = $state(new Map<string, IndexedImage>());
  let sprite2Loaded = $state(new Map<string, IndexedImage>());
  let bitmapLoadedPages = $state(new Set<number>());
  let spriteLoadedPages = $state(new Set<number>());
  let sprite2LoadedPages = $state(new Set<number>());
  let loadingPages = $state(new Set<string>());
  let bitmapName = $state('');
  let bitmapArchiveBytes: Uint8Array | undefined = $state();
  let bitmapArchiveUrl = $state('');
  let spriteName = $state('');
  let sprite2Name = $state('');
  let spriteArchiveBytes: Uint8Array | undefined = $state();
  let sprite2ArchiveBytes: Uint8Array | undefined = $state();
  let spriteArchiveUrl = $state('');
  let sprite2ArchiveUrl = $state('');
  let spriteEntries: GameOneArchiveEntry[] = $state([]);
  let sprite2Entries: GameOneArchiveEntry[] = $state([]);
  let bitmapEntries: GameOneArchiveEntry[] = $state([]);
  let originalSprites = $state(new Map<string, IndexedImage>());
  let originalSprites2 = $state(new Map<string, IndexedImage>());
  let originalBitmaps = $state(new Map<string, IndexedImage>());
  let modified = $state(new Set<string>());
  let modified2 = $state(new Set<string>());
  let modifiedBitmaps = $state(new Set<string>());
  let tab = $state<AssetTab>('bitmap');
  let page = $state(0);
  let jumpPage = $state('');
  let exportSelection = $state('0-95');
  let query = $state('');
  let paletteId = $state('1');
  let status = $state('Load bitmaps.dat, Sprite1.dat, or sprite2.dat from your game.');
  let error = $state('');
  let busy = $state(false);
  let dragging = $state(false);
  let selected: IndexedImage | undefined = $state();
  let mapActive = $state(false);
  let fitPreviews = $state(true);

  let current = $derived(tab === 'bitmap' ? bitmaps : tab === 'sprite1' ? sprites : sprites2);
  let currentCatalogue = $derived(
    tab === 'bitmap' ? bitmapCatalogue : tab === 'sprite1' ? spriteCatalogue : sprite2Catalogue
  );
  let activeLoaded = $derived(
    tab === 'bitmap' ? bitmapLoaded : tab === 'sprite1' ? spriteLoaded : sprite2Loaded
  );
  let activeModified = $derived(
    tab === 'bitmap' ? modifiedBitmaps : tab === 'sprite2' ? modified2 : modified
  );
  let activeOriginals = $derived(
    tab === 'bitmap' ? originalBitmaps : tab === 'sprite2' ? originalSprites2 : originalSprites
  );
  let activeEntries = $derived(
    tab === 'bitmap' ? bitmapEntries : tab === 'sprite2' ? sprite2Entries : spriteEntries
  );
  let activeArchive = $derived(
    tab === 'bitmap' ? bitmapArchiveBytes : tab === 'sprite2' ? sprite2ArchiveBytes : spriteArchiveBytes
  );
  let activeArchiveUrl = $derived(
    tab === 'bitmap' ? bitmapArchiveUrl : tab === 'sprite2' ? sprite2ArchiveUrl : spriteArchiveUrl
  );
  let activeName = $derived(tab === 'bitmap' ? bitmapName : tab === 'sprite2' ? sprite2Name : spriteName);
  let activeArchiveLabel = $derived(
    tab === 'bitmap' ? 'bitmaps.dat' : tab === 'sprite2' ? 'sprite2.dat' : 'Sprite1.dat'
  );
  let maxAssetId = $derived(
    Math.max(0, ...currentCatalogue.map((image) => Number(image.id)).filter((id) => Number.isInteger(id)))
  );
  let filtered = $derived(
    currentCatalogue.filter(
      (image) =>
        !query.trim() ||
        image.id.includes(query.trim()) ||
        `${image.width}x${image.height}`.includes(query.trim().toLowerCase())
    )
  );
  let pages = $derived(Math.max(1, Math.ceil(filtered.length / pageSize)));
  let visibleEntries = $derived(filtered.slice(page * pageSize, (page + 1) * pageSize));
  let visible = $derived(
    visibleEntries
      .map((entry) => activeLoaded.get(entry.id))
      .filter((image): image is IndexedImage => image !== undefined)
  );
  let chosenBitmap = $derived(
    bitmaps.find((image) => image.id === paletteId.padStart(3, '0') && image.palette)
  );
  let paletteChoices = $derived(bitmaps.filter((image) => image.palette));
  let chosenPalette: Palette = $derived(chosenBitmap?.palette ?? diagnosticPalette());

  function thumbnailScale(image: IndexedImage) {
    const largestSide = Math.max(image.width, image.height);
    return largestSide < 48 ? Math.min(3, 48 / largestSide) : 1;
  }

  function resetView(nextTab: AssetTab) {
    mapActive = false;
    tab = nextTab;
    page = 0;
    query = '';
  }

  function fromBase64(value: string | undefined) {
    if (!value) return undefined;
    const raw = atob(value);
    return Uint8Array.from(raw, (character) => character.charCodeAt(0));
  }

  function readPreparedImage(image: PreparedImage): IndexedImage {
    return {
      ...image,
      pixels: fromBase64(image.pixels)!,
      alpha: fromBase64(image.alpha),
      palette: fromBase64(image.palette),
      pcxHeader: fromBase64(image.pcxHeader)
    };
  }

  function setCatalogue(target: AssetTab, entries: CatalogueEntry[]) {
    if (target === 'bitmap') bitmapCatalogue = entries;
    else if (target === 'sprite2') sprite2Catalogue = entries;
    else spriteCatalogue = entries;
  }

  function registerPreparedImages(target: AssetTab, images: IndexedImage[]) {
    const pairs = images.map((image): [string, IndexedImage] => [image.id, image]);
    if (target === 'bitmap') {
      bitmapLoaded = new Map([...bitmapLoaded, ...pairs]);
      bitmaps = [...bitmapLoaded.values()];
      originalBitmaps = new Map([...originalBitmaps, ...pairs]);
    } else if (target === 'sprite2') {
      sprite2Loaded = new Map([...sprite2Loaded, ...pairs]);
      sprites2 = [...sprite2Loaded.values()];
      originalSprites2 = new Map([...originalSprites2, ...pairs]);
    } else {
      spriteLoaded = new Map([...spriteLoaded, ...pairs]);
      sprites = [...spriteLoaded.values()];
      originalSprites = new Map([...originalSprites, ...pairs]);
    }
  }

  function registerFullArchive(target: AssetTab, images: IndexedImage[]) {
    setCatalogue(
      target,
      images.map((image, index) => ({
        id: image.id,
        width: image.width,
        height: image.height,
        kind: image.kind,
        page: Math.floor(index / pageSize)
      }))
    );
    if (target === 'bitmap') {
      bitmapLoaded = new Map(images.map((image) => [image.id, image]));
      bitmapLoadedPages = new Set(images.map((_, index) => Math.floor(index / pageSize)));
    } else if (target === 'sprite2') {
      sprite2Loaded = new Map(images.map((image) => [image.id, image]));
      sprite2LoadedPages = new Set(images.map((_, index) => Math.floor(index / pageSize)));
    } else {
      spriteLoaded = new Map(images.map((image) => [image.id, image]));
      spriteLoadedPages = new Set(images.map((_, index) => Math.floor(index / pageSize)));
    }
    registerPreparedImages(target, images);
  }

  async function loadBytes(bytes: Uint8Array, fileName: string, spriteTarget?: 'sprite1' | 'sprite2') {
    error = '';
    status = `Reading ${fileName}…`;
    await new Promise((resolve) => setTimeout(resolve, 0));
    try {
      const name = fileName.toLowerCase();
      if (name.includes('sprite')) {
        const result = readSprites(bytes);
        const target = spriteTarget ?? (name.includes('sprite2') ? 'sprite2' : 'sprite1');
        if (target === 'sprite2') {
          sprites2 = result.images;
          sprite2ArchiveBytes = bytes;
          sprite2ArchiveUrl = '';
          sprite2Entries = result.entries;
          originalSprites2 = new Map(result.images.map((image) => [image.id, image]));
          modified2 = new Set();
          sprite2Name = fileName;
          originalSprites2 = new Map();
          registerFullArchive('sprite2', result.images);
        } else {
          sprites = result.images;
          spriteArchiveBytes = bytes;
          spriteArchiveUrl = '';
          spriteEntries = result.entries;
          originalSprites = new Map(result.images.map((image) => [image.id, image]));
          modified = new Set();
          spriteName = fileName;
          originalSprites = new Map();
          registerFullArchive('sprite1', result.images);
        }
        resetView(target);
        status = `Decoded ${result.images.length.toLocaleString()} sprites from ${fileName}.`;
      } else if (name.includes('bitmap')) {
        const result = readBitmaps(bytes);
        bitmaps = result.images;
        bitmapArchiveBytes = bytes;
        bitmapArchiveUrl = '';
        bitmapEntries = result.entries;
        originalBitmaps = new Map(result.images.map((image) => [image.id, image]));
        modifiedBitmaps = new Set();
        bitmapName = fileName;
        originalBitmaps = new Map();
        registerFullArchive('bitmap', result.images);
        resetView('bitmap');
        status = `Decoded ${bitmaps.length.toLocaleString()} PCX bitmaps from ${fileName}.`;
      } else {
        const bitmapResult = readBitmaps(bytes);
        if (bitmapResult.images.length) {
          bitmaps = bitmapResult.images;
          bitmapArchiveBytes = bytes;
          bitmapArchiveUrl = '';
          bitmapEntries = bitmapResult.entries;
          originalBitmaps = new Map(bitmapResult.images.map((image) => [image.id, image]));
          modifiedBitmaps = new Set();
          bitmapName = fileName;
          originalBitmaps = new Map();
          registerFullArchive('bitmap', bitmapResult.images);
          resetView('bitmap');
          status = `Detected ${bitmapResult.images.length.toLocaleString()} PCX bitmaps.`;
        } else {
          const spriteResult = readSprites(bytes);
          if (!spriteResult.images.length)
            throw new Error('This is a GameOne archive, but no supported PCX bitmaps or sprites were found.');
          sprites = spriteResult.images;
          spriteArchiveBytes = bytes;
          spriteArchiveUrl = '';
          spriteEntries = spriteResult.entries;
          originalSprites = new Map(spriteResult.images.map((image) => [image.id, image]));
          modified = new Set();
          spriteName = fileName;
          originalSprites = new Map();
          registerFullArchive('sprite1', spriteResult.images);
          resetView('sprite1');
          status = `Detected ${spriteResult.images.length.toLocaleString()} sprites.`;
        }
      }
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
      status = 'Loading failed.';
    }
  }

  function preparedName(target: AssetTab) {
    return target === 'bitmap' ? 'bitmaps.dat' : target === 'sprite2' ? 'sprite2.dat' : 'Sprite1.dat';
  }

  function loadedPages(target: AssetTab) {
    return target === 'bitmap'
      ? bitmapLoadedPages
      : target === 'sprite2'
        ? sprite2LoadedPages
        : spriteLoadedPages;
  }

  async function loadPreparedPage(target: AssetTab, preparedPage: number) {
    const key = `${target}:${preparedPage}`;
    if (loadedPages(target).has(preparedPage) || loadingPages.has(key)) return;
    loadingPages = new Set([...loadingPages, key]);
    try {
      const response = await fetch(`/game/prepared/${preparedName(target)}/page-${preparedPage}.prepared`);
      if (!response.ok) throw new Error(`prepared page returned HTTP ${response.status}`);
      const body = response.body;
      if (!body) throw new Error('prepared page has no response body');
      const inflated = await new Response(body.pipeThrough(new DecompressionStream('gzip'))).arrayBuffer();
      const prepared = JSON.parse(new TextDecoder().decode(inflated)) as {
        version: number;
        images: PreparedImage[];
      };
      if (prepared.version !== 1 || !Array.isArray(prepared.images))
        throw new Error('prepared page is invalid');
      registerPreparedImages(target, prepared.images.map(readPreparedImage));
      if (target === 'bitmap') bitmapLoadedPages = new Set([...bitmapLoadedPages, preparedPage]);
      else if (target === 'sprite2') sprite2LoadedPages = new Set([...sprite2LoadedPages, preparedPage]);
      else spriteLoadedPages = new Set([...spriteLoadedPages, preparedPage]);
    } catch (cause) {
      error = `Prepared ${preparedName(target)} page could not load: ${cause instanceof Error ? cause.message : String(cause)}`;
    } finally {
      loadingPages = new Set([...loadingPages].filter((value) => value !== key));
    }
  }

  async function loadVisiblePreparedPage() {
    const pagesToLoad = new Set(visibleEntries.map((entry) => entry.page));
    await Promise.all([...pagesToLoad].map((preparedPage) => loadPreparedPage(tab, preparedPage)));
  }

  async function ensurePreparedAsset(target: AssetTab, id: string) {
    const catalogue =
      target === 'bitmap' ? bitmapCatalogue : target === 'sprite2' ? sprite2Catalogue : spriteCatalogue;
    const entry = catalogue.find((candidate) => candidate.id === id);
    if (!entry) return undefined;
    await loadPreparedPage(target, entry.page);
    return target === 'bitmap'
      ? bitmapLoaded.get(id)
      : target === 'sprite2'
        ? sprite2Loaded.get(id)
        : spriteLoaded.get(id);
  }

  async function loadPreparedCatalogue() {
    try {
      const response = await fetch('/game/prepared/catalogue.json');
      if (!response.ok) return;
      const catalogue = (await response.json()) as PreparedCatalogue;
      if (catalogue.version !== 1 || !Array.isArray(catalogue.archives))
        throw new Error('prepared catalogue is invalid');
      for (const archive of catalogue.archives) {
        if (archive.name === 'bitmaps.dat') setCatalogue('bitmap', archive.entries);
        else if (archive.name === 'Sprite1.dat') setCatalogue('sprite1', archive.entries);
        else if (archive.name === 'sprite2.dat') setCatalogue('sprite2', archive.entries);
      }
      bitmapName = 'bitmaps.dat';
      spriteName = 'Sprite1.dat';
      sprite2Name = 'sprite2.dat';
      bitmapArchiveUrl = '/game/bitmaps.dat';
      spriteArchiveUrl = '/game/Sprite1.dat';
      sprite2ArchiveUrl = '/game/sprite2.dat';
      status = `Prepared catalogues ready: ${bitmapCatalogue.length.toLocaleString()} bitmaps, ${spriteCatalogue.length.toLocaleString()} anchored sprites, and ${sprite2Catalogue.length.toLocaleString()} UI sprites.`;
      void loadVisiblePreparedPage();
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
      status = 'Prepared catalogue loading failed.';
    }
  }

  onMount(() => void loadPreparedCatalogue());

  $effect(() => {
    if (visibleEntries.length) void loadVisiblePreparedPage();
  });

  async function archiveInput(event: Event, target?: 'sprite1' | 'sprite2') {
    const input = event.currentTarget as HTMLInputElement;
    if (input.files?.[0])
      await loadBytes(new Uint8Array(await input.files[0].arrayBuffer()), input.files[0].name, target);
    input.value = '';
  }

  function dropArchives(event: DragEvent) {
    event.preventDefault();
    const files = Array.from(event.dataTransfer?.files ?? []);
    for (const file of files) {
      const name = file.name.toLowerCase();
      if (name === 'bitmaps.dat')
        void file.arrayBuffer().then((bytes) => loadBytes(new Uint8Array(bytes), file.name));
      else if (name === 'sprite1.dat')
        void file.arrayBuffer().then((bytes) => loadBytes(new Uint8Array(bytes), file.name, 'sprite1'));
      else if (name === 'sprite2.dat')
        void file.arrayBuffer().then((bytes) => loadBytes(new Uint8Array(bytes), file.name, 'sprite2'));
    }
    if (
      !files.some((file) => ['bitmaps.dat', 'sprite1.dat', 'sprite2.dat'].includes(file.name.toLowerCase()))
    )
      error = 'Drop bitmaps.dat, Sprite1.dat, or sprite2.dat.';
  }
  function go(delta: number) {
    page = Math.max(0, Math.min(pages - 1, page + delta));
  }
  function jumpToPage() {
    const requested = Number.parseInt(String(jumpPage), 10);
    if (Number.isFinite(requested)) page = Math.max(0, Math.min(pages - 1, requested - 1));
    jumpPage = '';
  }

  function sameBytes(left: Uint8Array, right: Uint8Array) {
    return left.length === right.length && left.every((byte, index) => byte === right[index]);
  }

  async function ensureActiveArchive() {
    if (activeArchive?.length) return activeArchive;
    if (!activeArchiveUrl) throw new Error(`Load the original ${activeArchiveLabel} first.`);
    status = `Loading ${activeArchiveLabel} for verified export…`;
    const response = await fetch(activeArchiveUrl);
    if (!response.ok) throw new Error(`${activeArchiveLabel} returned HTTP ${response.status}.`);
    const archive = new Uint8Array(await response.arrayBuffer());
    if (tab === 'bitmap') bitmapArchiveBytes = archive;
    else if (tab === 'sprite2') sprite2ArchiveBytes = archive;
    else spriteArchiveBytes = archive;
    return archive;
  }

  function normaliseImportedPalette(original: IndexedImage, png: IndexedPng) {
    const safe = new Set<number>();
    for (let pixel = 0; pixel < original.pixels.length; pixel += 1)
      if (original.alpha?.[pixel]) safe.add(original.pixels[pixel]);
    if (!safe.size)
      throw new Error(`Original sprite ${original.id} has no visible palette slots to preserve.`);
    const slots = [...safe].sort((left, right) => left - right);
    const pixels = png.pixels.slice();
    let remapped = 0;
    for (let pixel = 0; pixel < pixels.length; pixel += 1) {
      if (!png.alpha[pixel]) {
        pixels[pixel] = 0;
        continue;
      }
      const source = pixels[pixel];
      if (safe.has(source)) continue;
      let closest = slots[0],
        distance = Number.POSITIVE_INFINITY;
      for (const candidate of slots) {
        const red = png.palette[source * 3] - png.palette[candidate * 3];
        const green = png.palette[source * 3 + 1] - png.palette[candidate * 3 + 1];
        const blue = png.palette[source * 3 + 2] - png.palette[candidate * 3 + 2];
        const nextDistance = red * red + green * green + blue * blue;
        if (nextDistance < distance) {
          closest = candidate;
          distance = nextDistance;
        }
      }
      pixels[pixel] = closest;
      remapped += 1;
    }
    return { pixels, remapped };
  }

  function selectedIds() {
    const tokens = exportSelection
      .trim()
      .split(/[\s,]+/)
      .filter(Boolean);
    if (!tokens.length) throw new Error('Enter one or more IDs, such as 1-10, 15.');
    const selected = new Set<number>();
    for (const token of tokens) {
      const match = /^(\d+)(?:\s*-\s*(\d+))?$/.exec(token);
      if (!match) throw new Error(`“${token}” is not an ID or inclusive ID range.`);
      const from = Number(match[1]);
      const to = Number(match[2] ?? match[1]);
      if (!Number.isInteger(from) || !Number.isInteger(to) || from < 0 || to < from || to > maxAssetId)
        throw new Error(`IDs must be whole numbers from 0 through ${maxAssetId}.`);
      for (let id = from; id <= to; id += 1) selected.add(id);
    }
    return [...selected].sort((left, right) => left - right);
  }

  async function exportIndexedRange() {
    error = '';
    try {
      if (tab !== 'bitmap' && !chosenBitmap?.palette)
        throw new Error('Load bitmaps.dat and select a valid bitmap palette before exporting sprites.');
      const ids = selectedIds();
      const selectedSprites = (
        await Promise.all(ids.map((id) => ensurePreparedAsset(tab, String(id).padStart(3, '0'))))
      ).filter((image): image is IndexedImage => image !== undefined);
      if (!selectedSprites.length) throw new Error('No decoded assets exist in that selection.');
      busy = true;
      const entries: { name: string; bytes: Uint8Array }[] = [];
      for (let index = 0; index < selectedSprites.length; index += 1) {
        const image = selectedSprites[index];
        const alpha = image.alpha ?? new Uint8Array(image.pixels.length).fill(255);
        const transparent = image.kind === 'sprite' ? transparencyIndex(image.pixels, alpha) : undefined;
        entries.push({
          name: `${image.id}.png`,
          bytes: await encodeIndexedPng(
            {
              width: image.width,
              height: image.height,
              pixels: image.pixels,
              alpha,
              palette: image.kind === 'bitmap' ? image.palette! : chosenBitmap!.palette!
            },
            transparent
          )
        });
        if (index % 25 === 0) {
          status = `Encoding indexed ${tab === 'bitmap' ? 'bitmap' : 'sprite'} ${index + 1} / ${selectedSprites.length}…`;
          await new Promise((resolve) => setTimeout(resolve, 0));
        }
      }
      downloadBlob(storedZip(entries), `${tab}-selected.zip`);
      const skipped = ids.length - selectedSprites.length;
      status = `Exported ${entries.length} indexed PNGs${tab === 'bitmap' ? '' : ` using bitmap ${chosenBitmap!.id}`}${skipped ? `; skipped ${skipped} missing or unsupported records` : ''}.`;
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
      status = 'Asset PNG export failed.';
    } finally {
      busy = false;
    }
  }

  async function importIndexedPngs(files: FileList | File[]) {
    error = '';
    const incoming = Array.from(files);
    const named = incoming.map((file) => {
      const match = /^(\d+)\.png$/i.exec(file.name);
      const numeric = match ? Number(match[1]) : NaN;
      return {
        file,
        id:
          Number.isInteger(numeric) && numeric >= 0 && numeric <= maxAssetId
            ? String(numeric).padStart(3, '0')
            : ''
      };
    });
    const counts = new Map<string, number>();
    for (const item of named) if (item.id) counts.set(item.id, (counts.get(item.id) ?? 0) + 1);
    const rejected: string[] = [];
    let applied = 0;
    let normalised = 0;
    let resized = 0;
    busy = true;
    try {
      for (const item of named) {
        if (!item.id) {
          rejected.push(`${item.file.name}: filename must be an asset number followed by .png`);
          continue;
        }
        if ((counts.get(item.id) ?? 0) > 1) {
          rejected.push(`${item.file.name}: duplicate asset ID ${Number(item.id)}`);
          continue;
        }
        const original = activeOriginals.get(item.id) ?? (await ensurePreparedAsset(tab, item.id));
        if (!original) {
          rejected.push(`${item.file.name}: ID is not a decoded ${tab === 'bitmap' ? 'bitmap' : 'sprite'}`);
          continue;
        }
        try {
          const png = await decodeIndexedPng(new Uint8Array(await item.file.arrayBuffer()));
          if (png.width > 2048 || png.height > 2048)
            throw new Error(
              `dimensions ${png.width}×${png.height} exceed the sprite format limit of 2048×2048`
            );
          let replacement: IndexedImage;
          if (tab === 'bitmap') {
            if (png.width !== original.width || png.height !== original.height)
              throw new Error(`bitmap dimensions must remain ${original.width}×${original.height}`);
            if (png.alpha.some((value) => value !== 255))
              throw new Error('PCX bitmaps do not support transparent PNG pixels.');
            replacement = { ...original, pixels: png.pixels, palette: png.palette };
            replacement.byteLength = encodePcx(replacement).length;
            bitmaps = bitmaps.map((image) => (image.id === item.id ? replacement : image));
            bitmapLoaded = new Map([...bitmapLoaded, [item.id, replacement]]);
            modifiedBitmaps = new Set([...modifiedBitmaps, item.id]);
          } else {
            // Sprite archives contain no palette. Retain only slots proven by
            // the original sprite and map unfamiliar imported slots to one of
            // those slots so a PNG palette reorder cannot alter game artwork.
            const normalisedPixels = normaliseImportedPalette(original, png);
            replacement = {
              ...original,
              width: png.width,
              height: png.height,
              pixels: normalisedPixels.pixels,
              alpha: png.alpha
            };
            replacement.byteLength = encodeSprite(replacement).length;
            normalised += normalisedPixels.remapped;
            if (png.width !== original.width || png.height !== original.height) resized += 1;
          }
          if (tab === 'sprite2') {
            sprites2 = sprites2.map((image) => (image.id === item.id ? replacement : image));
            sprite2Loaded = new Map([...sprite2Loaded, [item.id, replacement]]);
            modified2 = new Set([...modified2, item.id]);
          } else if (tab === 'sprite1') {
            sprites = sprites.map((image) => (image.id === item.id ? replacement : image));
            spriteLoaded = new Map([...spriteLoaded, [item.id, replacement]]);
            modified = new Set([...modified, item.id]);
          }
          applied += 1;
        } catch (cause) {
          rejected.push(`${item.file.name}: ${cause instanceof Error ? cause.message : String(cause)}`);
        }
      }
      status = `Applied ${applied} indexed ${tab === 'bitmap' ? 'bitmap' : 'sprite'} replacement${applied === 1 ? '' : 's'}${resized ? `; accepted ${resized} resized sprite${resized === 1 ? '' : 's'}${tab === 'sprite1' ? ' with unchanged hotspots' : ''}` : ''}${normalised ? `; normalised ${normalised.toLocaleString()} unsafe palette pixels` : ''}${rejected.length ? `; rejected ${rejected.length}` : ''}.`;
      if (rejected.length) error = rejected.join('\n');
    } finally {
      busy = false;
      dragging = false;
    }
  }

  function importInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    if (input.files?.length) void importIndexedPngs(input.files);
    input.value = '';
  }

  function dropPngs(event: DragEvent) {
    event.preventDefault();
    dragging = false;
    if (event.dataTransfer?.files.length) void importIndexedPngs(event.dataTransfer.files);
  }

  async function exportModifiedAssets() {
    error = '';
    try {
      if (!activeModified.size) throw new Error('No replacements have been imported.');
      busy = true;
      status = `Rebuilding ${activeModified.size} modified ${tab === 'bitmap' ? 'bitmap' : 'sprite'} records…`;
      await new Promise((resolve) => setTimeout(resolve, 0));
      const archive = await ensureActiveArchive();
      // Pre-prepared dev data intentionally omits packed archive records to
      // keep the initial page responsive. Decode them only when an export
      // needs byte-for-byte untouched-record verification.
      const sourceEntries = activeEntries.length ? activeEntries : extractGameOneArchive(archive);
      const currentImages = new Map(current.map((image) => [image.id, image]));
      const replacements = new Map<string, Uint8Array>();
      for (const id of activeModified) {
        const image = currentImages.get(id)!;
        replacements.set(id, image.kind === 'bitmap' ? encodePcx(image) : encodeSprite(image));
      }
      const rebuilt = rebuildGameOneArchive(archive, replacements);
      const rebuiltArchive = extractGameOneArchive(rebuilt);
      const rebuiltEntries = new Map(rebuiltArchive.map((entry) => [entry.id, entry]));
      if (rebuiltArchive.length !== sourceEntries.length)
        throw new Error('Rebuilt archive changed the record count.');
      for (const original of sourceEntries) {
        if (
          !activeModified.has(original.id) &&
          !sameBytes(original.packed, rebuiltEntries.get(original.id)?.packed ?? new Uint8Array())
        ) {
          throw new Error(`Untouched record ${original.id} changed unexpectedly.`);
        }
      }
      const verified = new Map(
        (tab === 'bitmap' ? readBitmaps(rebuilt).images : readSprites(rebuilt).images).map((image) => [
          image.id,
          image
        ])
      );
      for (const id of activeModified) {
        const expected = currentImages.get(id)!,
          actual = verified.get(id);
        if (
          !actual ||
          actual.width !== expected.width ||
          actual.height !== expected.height ||
          !sameBytes(actual.pixels, expected.pixels) ||
          (expected.kind === 'sprite' &&
            (actual.magic !== expected.magic ||
              actual.hotspotX !== expected.hotspotX ||
              actual.hotspotY !== expected.hotspotY ||
              !sameBytes(actual.alpha!, expected.alpha!))) ||
          (expected.kind === 'bitmap' && !sameBytes(actual.palette!, expected.palette!))
        ) {
          throw new Error(`${tab === 'bitmap' ? 'Bitmap' : 'Sprite'} verification failed for ${id}.`);
        }
      }
      const outputName =
        tab === 'bitmap'
          ? 'bitmaps-modified.dat'
          : tab === 'sprite2'
            ? 'sprite2-modified.dat'
            : 'Sprite1-modified.dat';
      downloadBlob(binaryBlob(rebuilt), outputName);
      status = `Exported and verified ${outputName} with ${activeModified.size} modified ${tab === 'bitmap' ? 'bitmaps' : 'sprites'}.`;
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
      status = 'Asset archive export failed.';
    } finally {
      busy = false;
    }
  }
</script>

<div class="asset-viewer">
  <main>
    <header class="studio-hero">
      <div>
        <p class="eyebrow">DORAEMON MONOPOLY</p>
        <h1>Graphics studio</h1>
        <p class="lede">
          Inspect, export, replace, and rebuild the game's indexed bitmap and sprite resources.
        </p>
      </div>
      <nav class="studio-switcher" aria-label="Resource studios">
        <a href="/" data-route>Translation</a><a class="active" href="/assets" data-route>Graphics</a><a
          href="/fonts"
          data-route>Fonts</a
        >
      </nav>
    </header>

    <section class="explanation">
      <article>
        <strong>bitmaps.dat</strong><span
          >174 archive records in this release. Most drawable entries are complete 8-bit PCX screens,
          backgrounds, portraits, and artwork. Chinese UI text may already be baked into these pixels.</span
        >
      </article>
      <article>
        <strong>Sprite1.dat</strong><span
          >13,807 small transparent overlays in a custom scanline format. These are assembled over bitmaps by
          the game and borrow a 256-color palette from screen artwork.</span
        >
      </article>
      <article>
        <strong>sprite2.dat</strong><span
          >727 overlays using the same indexed RLE scanlines as Sprite1, but with the 0x8002 header variant:
          it has no hotspot fields, so its row-offset table begins four bytes earlier.</span
        >
      </article>
    </section>

    <section
      class="load-panel"
      aria-label="Asset loaders"
      ondragover={(event) => event.preventDefault()}
      ondrop={dropArchives}
    >
      <strong>Your local game archives</strong>
      <span
        >Choose or drop your own files. Optional ignored copies in <code>public/game</code> load automatically during
        local development; nothing is uploaded.</span
      >
      <div>
        <label class="file-button"
          >Load bitmaps.dat<input
            type="file"
            accept=".dat,application/octet-stream"
            onchange={(event) => archiveInput(event)}
          /></label
        >
        <label class="file-button"
          >Load Sprite1.dat<input
            type="file"
            accept=".dat,application/octet-stream"
            onchange={(event) => archiveInput(event, 'sprite1')}
          /></label
        >
        <label class="file-button"
          >Load sprite2.dat<input
            type="file"
            accept=".dat,application/octet-stream"
            onchange={(event) => archiveInput(event, 'sprite2')}
          /></label
        >
      </div>
    </section>

    {#if error}<p class="error">{error}</p>{/if}
    <p class="status">{status}</p>

    <nav class="tabs" aria-label="Asset type">
      <button class:active={tab === 'bitmap'} onclick={() => resetView('bitmap')}
        >Bitmap screens <b>{bitmapCatalogue.length.toLocaleString()}</b></button
      >
      <button class:active={tab === 'sprite1'} onclick={() => resetView('sprite1')}
        >Anchored sprites <b>{spriteCatalogue.length.toLocaleString()}</b></button
      >
      <button class:active={tab === 'sprite2'} onclick={() => resetView('sprite2')}
        >UI sprites <b>{sprite2Catalogue.length.toLocaleString()}</b></button
      >
      <button class:active={mapActive} onclick={() => (mapActive = true)}>Maps</button>
    </nav>

    {#if mapActive}
      <MapStudio />
    {:else}
      <section class="toolbar">
        <label
          >Find entry or size<input
            type="search"
            placeholder="e.g. 053 or 640x480"
            bind:value={query}
            oninput={() => (page = 0)}
          /></label
        >
        {#if tab !== 'bitmap'}
          <label
            >Preview palette
            <select class="palette-id" bind:value={paletteId} disabled={!paletteChoices.length}>
              {#if !paletteChoices.length}<option value="1">Load bitmaps.dat first</option>{/if}
              {#each paletteChoices as bitmap (bitmap.id)}<option value={String(Number(bitmap.id))}
                  >Bitmap #{bitmap.id} · {bitmap.width}×{bitmap.height}</option
                >{/each}
            </select>
          </label>
          <span class="palette-state"
            >{chosenBitmap
              ? `Using the 256-color palette embedded in bitmap ${chosenBitmap.id}`
              : 'Using diagnostic colors'}</span
          >
        {/if}
        <label class="fit-previews"
          ><span>Fit artwork</span><input type="checkbox" bind:checked={fitPreviews} /></label
        >
      </section>

      {#if currentCatalogue.length}
        <section class="sprite-editor" aria-label="Indexed sprite editor">
          <div class="sprite-export">
            <div>
              <strong>Export indexed PNGs</strong><span
                >Use IDs or inclusive ranges: <code>1-10, 15</code>, <code>1 2 4 5</code>, or
                <code>1-2, 4-5</code>.</span
              >
            </div>
            <label
              >IDs<input
                type="text"
                inputmode="numeric"
                placeholder="e.g. 1-10, 15"
                bind:value={exportSelection}
              /></label
            >
            <button
              type="button"
              disabled={busy || (tab !== 'bitmap' && !chosenBitmap?.palette)}
              onclick={exportIndexedRange}>Export PNG ZIP</button
            >
          </div>
          <div
            class:dragging
            class="sprite-import"
            role="group"
            aria-label="Indexed sprite PNG import"
            ondragover={(event) => {
              event.preventDefault();
              dragging = true;
            }}
            ondragleave={() => (dragging = false)}
            ondrop={dropPngs}
          >
            <div>
              <strong>Import Aseprite replacements</strong><span
                >Drop numbered indexed PNGs here, or choose several files. RGB/RGBA images are rejected.
                {#if tab === 'bitmap'}Bitmap dimensions and opaque pixels must stay unchanged. Its imported
                  256-color palette becomes the palette used by sprites that select this bitmap.{:else}Resizing
                  is supported by rebuilding the dimensions and every row offset; Sprite1 retains its original
                  hotspot, while Sprite2 has no hotspot fields.{/if}</span
              >
            </div>
            <label class="file-button"
              >Choose PNGs<input type="file" accept="image/png,.png" multiple onchange={importInput} /></label
            >
          </div>
          <div class="aseprite-note">
            <strong>Aseprite safety</strong><span
              >Keep <b>Indexed</b> color mode. {#if tab === 'bitmap'}Changing a bitmap palette is intentional
                and can recolor any sprite rendered with that bitmap’s palette. Transparent pixels are not
                supported by PCX bitmaps.{:else}Keep palette order and the transparent slot. Resizing is
                experimental: the original hotspot is preserved unchanged, so the game may shift, clip, or
                reject the sprite. Reordering palette colors changes the game’s pixel indices even when the
                image still looks correct.{/if}</span
            >
          </div>
          {#if tab === 'sprite2'}<div class="aseprite-note">
              <strong>Sprite2 header</strong><span
                >The executable confirms that 0x8002 means indexed RLE with no hotspot. Its row table starts
                at byte 6 instead of byte 10; there is no extra zero opcode or hidden geometry footer.</span
              >
            </div>{/if}
          <div class="sprite-save">
            <span
              ><b>{activeModified.size}</b> modified {tab === 'bitmap'
                ? 'bitmap'
                : 'sprite'}{activeModified.size === 1 ? '' : 's'}. {#if tab === 'bitmap'}Its edited palette is
                retained in bitmaps.dat and immediately used by sprite previews when selected.{:else}Palette
                RGB is preview-only; visible colours are constrained to palette slots proven by the original
                sprite.{/if}</span
            ><button
              type="button"
              class="primary"
              disabled={busy || !activeModified.size}
              onclick={exportModifiedAssets}>Export modified {activeArchiveLabel}</button
            >
          </div>
        </section>
      {/if}

      {#if !currentCatalogue.length}
        <section class="empty">
          Load {tab === 'bitmap' ? 'bitmaps.dat' : activeArchiveLabel} to inspect it here.
        </section>
      {:else}
        <p class="count">
          Showing {visible.length} of {filtered.length.toLocaleString()} loaded {tab === 'bitmap'
            ? 'bitmaps'
            : 'sprites'} · {tab === 'bitmap' ? bitmapName : activeName}
        </p>
        <section class="grid">
          {#each visible as image (image.id)}
            <AssetTile
              {image}
              palette={chosenPalette}
              fitVisible={fitPreviews}
              scale={thumbnailScale(image)}
              modified={activeModified.has(image.id)}
              onopen={() => (selected = image)}
            />
          {/each}
        </section>
      {/if}

      <nav class="bottom-nav" aria-label="Asset page navigation">
        <span>Page <b>{page + 1}</b> / {pages}</span>
        <button aria-label="Previous page" onclick={() => go(-1)} disabled={page === 0}>←</button>
        <button aria-label="Next page" onclick={() => go(1)} disabled={page + 1 >= pages}>→</button>
        {#if tab !== 'bitmap'}
          <form
            class="page-jump"
            onsubmit={(event) => {
              event.preventDefault();
              jumpToPage();
            }}
          >
            <label
              >Go to page<input
                type="number"
                min="1"
                max={pages}
                placeholder="1–{pages}"
                bind:value={jumpPage}
              /></label
            >
            <button type="submit" disabled={!String(jumpPage).trim()}>Go</button>
          </form>
        {/if}
      </nav>
      {#if selected}
        <div class="modal">
          <div
            class="modal-panel"
            role="dialog"
            aria-modal="true"
            aria-label={`Asset ${selected.id}`}
            tabindex="-1"
          >
            <header>
              <div>
                <strong>#{selected.id}</strong><span
                  >{selected.kind} · {selected.width} × {selected.height} · {selected.byteLength.toLocaleString()}
                  decoded bytes</span
                >{#if selected.kind === 'sprite'}<span
                    >{selected.hotspotX === undefined
                      ? 'No hotspot'
                      : `Hotspot ${selected.hotspotX}, ${selected.hotspotY}`} · format 0x{selected.magic?.toString(
                      16
                    )}</span
                  >{/if}
              </div>
              <button onclick={() => (selected = undefined)}>Close</button>
            </header>
            <div class="large-preview"><IndexedCanvas image={selected} palette={chosenPalette} /></div>
          </div>
        </div>
      {/if}
    {/if}
  </main>
</div>
