<script lang="ts">
  import './asset-viewer.css';
  import AssetCanvas from './AssetCanvas.svelte';
  import { diagnosticPalette, encodeSprite, readBitmaps, readSprites, type IndexedImage, type Palette } from './lib/asset-formats';
  import { extractGameOneArchive, rebuildGameOneArchive, type GameOneArchiveEntry } from './lib/formats';
  import { decodeIndexedPng, encodeIndexedPng, transparencyIndex, type IndexedPng } from './lib/indexed-png';
  import { storedZip } from './lib/stored-zip';

  const pageSize = 96;
  let bitmaps: IndexedImage[] = $state([]);
  let sprites: IndexedImage[] = $state([]);
  let sprites2: IndexedImage[] = $state([]);
  let bitmapName = $state('');
  let spriteName = $state('');
  let sprite2Name = $state('');
  let spriteArchiveBytes: Uint8Array | undefined = $state();
  let sprite2ArchiveBytes: Uint8Array | undefined = $state();
  let spriteEntries: GameOneArchiveEntry[] = $state([]);
  let sprite2Entries: GameOneArchiveEntry[] = $state([]);
  let originalSprites = $state(new Map<string, IndexedImage>());
  let originalSprites2 = $state(new Map<string, IndexedImage>());
  let modified = $state(new Set<string>());
  let modified2 = $state(new Set<string>());
  let tab: 'bitmap' | 'sprite1' | 'sprite2' = $state('bitmap');
  let page = $state(0);
  let jumpPage = $state('');
  let exportFrom = $state('0');
  let exportTo = $state('95');
  let query = $state('');
  let paletteId = $state('1');
  let status = $state('Load the bundled bitmap and sprite archives when needed.');
  let error = $state('');
  let busy = $state(false);
  let dragging = $state(false);
  let selected: IndexedImage | undefined = $state();

  let current = $derived(tab === 'bitmap' ? bitmaps : tab === 'sprite1' ? sprites : sprites2);
  let activeModified = $derived(tab === 'sprite2' ? modified2 : modified);
  let activeOriginals = $derived(tab === 'sprite2' ? originalSprites2 : originalSprites);
  let activeEntries = $derived(tab === 'sprite2' ? sprite2Entries : spriteEntries);
  let activeArchive = $derived(tab === 'sprite2' ? sprite2ArchiveBytes : spriteArchiveBytes);
  let activeName = $derived(tab === 'sprite2' ? sprite2Name : spriteName);
  let activeArchiveLabel = $derived(tab === 'sprite2' ? 'sprite2.dat' : 'Sprite1.dat');
  let maxSpriteId = $derived(Math.max(0, activeEntries.length - 1));
  let filtered = $derived(current.filter((image) => !query.trim() || image.id.includes(query.trim()) || `${image.width}x${image.height}`.includes(query.trim().toLowerCase())));
  let pages = $derived(Math.max(1, Math.ceil(filtered.length / pageSize)));
  let visible = $derived(filtered.slice(page * pageSize, (page + 1) * pageSize));
  let chosenBitmap = $derived(bitmaps.find((image) => image.id === paletteId.padStart(3, '0') && image.palette));
  let paletteChoices = $derived(bitmaps.filter((image) => image.palette));
  let chosenPalette: Palette = $derived(chosenBitmap?.palette ?? diagnosticPalette());

  function resetView(nextTab: 'bitmap' | 'sprite1' | 'sprite2') { tab = nextTab; page = 0; query = ''; }

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
          sprites2 = result.images; sprite2ArchiveBytes = bytes; sprite2Entries = result.entries;
          originalSprites2 = new Map(result.images.map((image) => [image.id, image])); modified2 = new Set(); sprite2Name = fileName;
        } else {
          sprites = result.images; spriteArchiveBytes = bytes; spriteEntries = result.entries;
          originalSprites = new Map(result.images.map((image) => [image.id, image])); modified = new Set(); spriteName = fileName;
        }
        resetView(target);
        status = `Decoded ${sprites.length.toLocaleString()} sprites from ${fileName}.`;
      } else if (name.includes('bitmap')) {
        const result = readBitmaps(bytes);
        bitmaps = result.images;
        bitmapName = fileName;
        resetView('bitmap');
        status = `Decoded ${bitmaps.length.toLocaleString()} PCX bitmaps from ${fileName}.`;
      } else {
        const bitmapResult = readBitmaps(bytes);
        if (bitmapResult.images.length) {
          bitmaps = bitmapResult.images; bitmapName = fileName; resetView('bitmap');
          status = `Detected ${bitmapResult.images.length.toLocaleString()} PCX bitmaps.`;
        } else {
          const spriteResult = readSprites(bytes);
          if (!spriteResult.images.length) throw new Error('This is a GameOne archive, but no supported PCX bitmaps or sprites were found.');
          sprites = spriteResult.images; spriteArchiveBytes = bytes; spriteEntries = spriteResult.entries;
          originalSprites = new Map(spriteResult.images.map((image) => [image.id, image])); modified = new Set(); spriteName = fileName; resetView('sprite1');
          status = `Detected ${spriteResult.images.length.toLocaleString()} sprites.`;
        }
      }
    } catch (cause) { error = cause instanceof Error ? cause.message : String(cause); status = 'Loading failed.'; }
  }

  async function loadBundled(url: string, name: string, target?: 'sprite1' | 'sprite2') {
    error = '';
    status = `Loading ${name}…`;
    try {
      const response = await fetch(url);
      if (!response.ok) throw new Error(`${name} returned HTTP ${response.status}.`);
      await loadBytes(new Uint8Array(await response.arrayBuffer()), name, target);
    } catch (cause) { error = cause instanceof Error ? cause.message : String(cause); status = 'Loading failed.'; }
  }
  function go(delta: number) { page = Math.max(0, Math.min(pages - 1, page + delta)); }
  function jumpToPage() {
    const requested = Number.parseInt(String(jumpPage), 10);
    if (Number.isFinite(requested)) page = Math.max(0, Math.min(pages - 1, requested - 1));
    jumpPage = '';
  }

  function download(blob: Blob, name: string) {
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a'); link.href = url; link.download = name; link.click();
    window.setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  function sameBytes(left: Uint8Array, right: Uint8Array) {
    return left.length === right.length && left.every((byte, index) => byte === right[index]);
  }

  function normaliseImportedPalette(original: IndexedImage, png: IndexedPng) {
    const safe = new Set<number>();
    for (let pixel = 0; pixel < original.pixels.length; pixel += 1) if (original.alpha?.[pixel]) safe.add(original.pixels[pixel]);
    if (!safe.size) throw new Error(`Original sprite ${original.id} has no visible palette slots to preserve.`);
    const slots = [...safe].sort((left, right) => left - right);
    const pixels = png.pixels.slice();
    let remapped = 0;
    for (let pixel = 0; pixel < pixels.length; pixel += 1) {
      if (!png.alpha[pixel]) { pixels[pixel] = 0; continue; }
      const source = pixels[pixel];
      if (safe.has(source)) continue;
      let closest = slots[0], distance = Number.POSITIVE_INFINITY;
      for (const candidate of slots) {
        const red = png.palette[source * 3] - png.palette[candidate * 3];
        const green = png.palette[source * 3 + 1] - png.palette[candidate * 3 + 1];
        const blue = png.palette[source * 3 + 2] - png.palette[candidate * 3 + 2];
        const nextDistance = red * red + green * green + blue * blue;
        if (nextDistance < distance) { closest = candidate; distance = nextDistance; }
      }
      pixels[pixel] = closest;
      remapped += 1;
    }
    return { pixels, remapped };
  }

  function range() {
    const from = Number(exportFrom), to = Number(exportTo);
    if (!Number.isInteger(from) || !Number.isInteger(to) || from < 0 || to > maxSpriteId || from > to) {
      throw new Error(`Sprite range must use whole IDs from 0 through ${maxSpriteId}, with From no greater than To.`);
    }
    return { from, to };
  }

  async function exportIndexedRange() {
    error = '';
    try {
      if (!chosenBitmap?.palette) throw new Error('Load bitmaps.dat and select a valid bitmap palette before exporting sprites.');
      const { from, to } = range();
      const selectedSprites = current.filter((image) => { const id = Number(image.id); return id >= from && id <= to; });
      if (!selectedSprites.length) throw new Error('No decoded sprites exist in that range.');
      busy = true;
      const entries: { name: string; bytes: Uint8Array }[] = [];
      for (let index = 0; index < selectedSprites.length; index += 1) {
        const image = selectedSprites[index];
        const alpha = image.alpha!;
        const transparent = transparencyIndex(image.pixels, alpha);
        entries.push({ name: `${image.id}.png`, bytes: await encodeIndexedPng({ width: image.width, height: image.height, pixels: image.pixels, alpha, palette: chosenBitmap.palette }, transparent) });
        if (index % 25 === 0) { status = `Encoding indexed sprite ${index + 1} / ${selectedSprites.length}…`; await new Promise((resolve) => setTimeout(resolve, 0)); }
      }
      download(storedZip(entries), `${tab}-${from}-${to}.zip`);
      const skipped = to - from + 1 - selectedSprites.length;
      status = `Exported ${entries.length} indexed PNGs using bitmap ${chosenBitmap.id}${skipped ? `; skipped ${skipped} non-sprite records` : ''}.`;
    } catch (cause) { error = cause instanceof Error ? cause.message : String(cause); status = 'Sprite PNG export failed.'; }
    finally { busy = false; }
  }

  async function importSpritePngs(files: FileList | File[]) {
    error = '';
    const incoming = Array.from(files);
    const named = incoming.map((file) => {
      const match = /^(\d+)\.png$/i.exec(file.name);
      const numeric = match ? Number(match[1]) : NaN;
      return { file, id: Number.isInteger(numeric) && numeric >= 0 && numeric <= maxSpriteId ? String(numeric).padStart(3, '0') : '' };
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
        if (!item.id) { rejected.push(`${item.file.name}: filename must be a sprite number followed by .png`); continue; }
        if ((counts.get(item.id) ?? 0) > 1) { rejected.push(`${item.file.name}: duplicate sprite ID ${Number(item.id)}`); continue; }
        const original = activeOriginals.get(item.id);
        if (!original) { rejected.push(`${item.file.name}: ID is not a decoded sprite`); continue; }
        try {
          const png = await decodeIndexedPng(new Uint8Array(await item.file.arrayBuffer()));
          if (png.width > 2048 || png.height > 2048) throw new Error(`dimensions ${png.width}×${png.height} exceed the sprite format limit of 2048×2048`);
          // Sprite1.dat has no embedded palette: retain only slots already
          // proven by the original sprite, mapping new colours to the nearest
          // safe slot. Transparent pixels have no stored palette index.
          const normalisedPixels = normaliseImportedPalette(original, png);
          const pixels = normalisedPixels.pixels;
          const replacement: IndexedImage = { ...original, width: png.width, height: png.height, pixels, alpha: png.alpha };
          replacement.byteLength = encodeSprite(replacement).length;
          if (tab === 'sprite2') { sprites2 = sprites2.map((image) => image.id === item.id ? replacement : image); modified2 = new Set([...modified2, item.id]); }
          else { sprites = sprites.map((image) => image.id === item.id ? replacement : image); modified = new Set([...modified, item.id]); }
          applied += 1;
          normalised += normalisedPixels.remapped;
          if (png.width !== original.width || png.height !== original.height) resized += 1;
        } catch (cause) { rejected.push(`${item.file.name}: ${cause instanceof Error ? cause.message : String(cause)}`); }
      }
      status = `Applied ${applied} indexed sprite replacement${applied === 1 ? '' : 's'}${resized ? `; accepted ${resized} resized sprite${resized === 1 ? '' : 's'}${tab === 'sprite1' ? ' with unchanged hotspots' : ''}` : ''}${normalised ? `; normalised ${normalised.toLocaleString()} unsafe palette pixels` : ''}${rejected.length ? `; rejected ${rejected.length}` : ''}.`;
      if (rejected.length) error = rejected.join('\n');
    } finally { busy = false; dragging = false; }
  }

  function importInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    if (input.files?.length) void importSpritePngs(input.files);
    input.value = '';
  }

  function dropPngs(event: DragEvent) {
    event.preventDefault(); dragging = false;
    if (event.dataTransfer?.files.length) void importSpritePngs(event.dataTransfer.files);
  }

  async function exportModifiedSprites() {
    error = '';
    try {
      if (!activeArchive || !activeEntries.length) throw new Error(`Load the original ${activeArchiveLabel} first.`);
      if (!activeModified.size) throw new Error('No sprite replacements have been imported.');
      busy = true; status = `Rebuilding ${activeModified.size} modified sprite records…`;
      await new Promise((resolve) => setTimeout(resolve, 0));
      const currentImages = new Map(current.map((image) => [image.id, image]));
      const replacements = new Map<string, Uint8Array>();
      for (const id of activeModified) replacements.set(id, encodeSprite(currentImages.get(id)!));
      const rebuilt = rebuildGameOneArchive(activeArchive, replacements);
      const rebuiltArchive = extractGameOneArchive(rebuilt);
      const rebuiltEntries = new Map(rebuiltArchive.map((entry) => [entry.id, entry]));
      if (rebuiltArchive.length !== activeEntries.length) throw new Error('Rebuilt archive changed the record count.');
      for (const original of activeEntries) {
        if (!activeModified.has(original.id) && !sameBytes(original.packed, rebuiltEntries.get(original.id)?.packed ?? new Uint8Array())) {
          throw new Error(`Untouched record ${original.id} changed unexpectedly.`);
        }
      }
      const verified = new Map(readSprites(rebuilt).images.map((image) => [image.id, image]));
      for (const id of activeModified) {
        const expected = currentImages.get(id)!, actual = verified.get(id);
        if (!actual || actual.width !== expected.width || actual.height !== expected.height || actual.magic !== expected.magic || actual.hotspotX !== expected.hotspotX || actual.hotspotY !== expected.hotspotY || !sameBytes(actual.pixels, expected.pixels) || !sameBytes(actual.alpha!, expected.alpha!)) {
          throw new Error(`Sprite verification failed for ${id}.`);
        }
      }
      const outputName = tab === 'sprite2' ? 'sprite2-modified.dat' : 'Sprite1-modified.dat';
      download(new Blob([rebuilt], { type: 'application/octet-stream' }), outputName);
      status = `Exported and verified ${outputName} with ${activeModified.size} modified sprites.`;
    } catch (cause) { error = cause instanceof Error ? cause.message : String(cause); status = 'Sprite archive export failed.'; }
    finally { busy = false; }
  }
</script>

<div class="asset-viewer">
<main>
  <header>
    <div><p class="eyebrow">DORAEMON MONOPOLY</p><h1>Asset viewer</h1><p class="lede">Inspect screen artwork and transparent sprite layers without changing the game files.</p></div>
    <a href="/" data-route>String translator</a>
  </header>

  <section class="explanation">
    <article><strong>bitmaps.dat</strong><span>174 archive records in this release. Most drawable entries are complete 8-bit PCX screens, backgrounds, portraits, and artwork. Chinese UI text may already be baked into these pixels.</span></article>
    <article><strong>Sprite1.dat</strong><span>13,807 small transparent overlays in a custom scanline format. These are assembled over bitmaps by the game and borrow a 256-color palette from screen artwork.</span></article>
    <article><strong>sprite2.dat</strong><span>727 overlays using the same indexed RLE scanlines as Sprite1, but with the 0x8002 header variant: it has no hotspot fields, so its row-offset table begins four bytes earlier.</span></article>
  </section>

  <section class="load-panel" aria-label="Asset loaders">
    <strong>Original game assets</strong>
    <span>Load the original archives bundled with this project; nothing is uploaded or modified.</span>
    <div><button type="button" onclick={() => loadBundled('/bitmaps.dat', 'bitmaps.dat')}>Load original bitmaps.dat</button><button type="button" onclick={() => loadBundled('/Sprite1.dat', 'Sprite1.dat', 'sprite1')}>Load original Sprite1.dat</button><button type="button" onclick={() => loadBundled('/sprite2.dat', 'sprite2.dat', 'sprite2')}>Load original sprite2.dat</button></div>
  </section>

  {#if error}<p class="error">{error}</p>{/if}
  <p class="status">{status}</p>

  <nav class="tabs" aria-label="Asset type">
    <button class:active={tab === 'bitmap'} onclick={() => resetView('bitmap')}>Bitmaps <b>{bitmaps.length.toLocaleString()}</b></button>
    <button class:active={tab === 'sprite1'} onclick={() => resetView('sprite1')}>Sprite1 <b>{sprites.length.toLocaleString()}</b></button>
    <button class:active={tab === 'sprite2'} onclick={() => resetView('sprite2')}>Sprite2 <b>{sprites2.length.toLocaleString()}</b></button>
  </nav>

  <section class="toolbar">
    <label>Find entry or size<input type="search" placeholder="e.g. 053 or 640x480" bind:value={query} oninput={() => page = 0} /></label>
    {#if tab !== 'bitmap'}
      <label>Preview palette
        <select class="palette-id" bind:value={paletteId} disabled={!paletteChoices.length}>
          {#if !paletteChoices.length}<option value="1">Load bitmaps.dat first</option>{/if}
          {#each paletteChoices as bitmap}<option value={String(Number(bitmap.id))}>Bitmap #{bitmap.id} · {bitmap.width}×{bitmap.height}</option>{/each}
        </select>
      </label>
      <span class="palette-state">{chosenBitmap ? `Using the 256-color palette embedded in bitmap ${chosenBitmap.id}` : 'Using diagnostic colors'}</span>
    {/if}
  </section>

  {#if tab !== 'bitmap' && current.length}
    <section class="sprite-editor" aria-label="Indexed sprite editor">
      <div class="sprite-export">
        <div><strong>Export indexed PNGs</strong><span>A real bitmap palette is required. The ID range is inclusive.</span></div>
        <label>From ID<input type="number" min="0" max={maxSpriteId} bind:value={exportFrom} /></label>
        <label>To ID<input type="number" min="0" max={maxSpriteId} bind:value={exportTo} /></label>
        <button type="button" disabled={busy || !chosenBitmap?.palette} onclick={exportIndexedRange}>Export PNG ZIP</button>
      </div>
      <div class:dragging class="sprite-import" role="group" aria-label="Indexed sprite PNG import" ondragover={(event) => { event.preventDefault(); dragging = true; }} ondragleave={() => dragging = false} ondrop={dropPngs}>
        <div><strong>Import Aseprite replacements</strong><span>Drop numbered indexed PNGs here, or choose several files. RGB/RGBA images are rejected. Resizing is supported by rebuilding the dimensions and every row offset; Sprite1 retains its original hotspot, while Sprite2 has no hotspot fields.</span></div>
        <label class="file-button">Choose PNGs<input type="file" accept="image/png,.png" multiple onchange={importInput} /></label>
      </div>
      <div class="aseprite-note"><strong>Aseprite safety</strong><span>Keep <b>Indexed</b> color mode, palette order, and transparent slot. Resizing is allowed but experimental: the original hotspot is preserved unchanged, so the game may shift, clip, or reject the sprite. Reordering palette colors changes the game’s pixel indices even when the image still looks correct.</span></div>
      {#if tab === 'sprite2'}<div class="aseprite-note"><strong>Sprite2 header</strong><span>The executable confirms that 0x8002 means indexed RLE with no hotspot. Its row table starts at byte 6 instead of byte 10; there is no extra zero opcode or hidden geometry footer.</span></div>{/if}
      <div class="sprite-save"><span><b>{activeModified.size}</b> modified sprite{activeModified.size === 1 ? '' : 's'}. Palette RGB is preview-only; visible colours are constrained to palette slots proven by the original sprite.</span><button type="button" class="primary" disabled={busy || !activeModified.size} onclick={exportModifiedSprites}>Export modified {activeArchiveLabel}</button></div>
    </section>
  {/if}

  {#if !current.length}
    <section class="empty">Load {tab === 'bitmap' ? 'bitmaps.dat' : activeArchiveLabel} to inspect it here.</section>
  {:else}
    <p class="count">Showing {visible.length} of {filtered.length.toLocaleString()} decoded {tab === 'bitmap' ? 'bitmaps' : 'sprites'} · {tab === 'bitmap' ? bitmapName : activeName}</p>
    <section class="grid">
      {#each visible as image (image.id)}
        <button class:modified={activeModified.has(image.id)} class="asset" onclick={() => selected = image} title={`Open ${image.id}`}>
          <span class="preview"><AssetCanvas {image} palette={chosenPalette} /></span>
          <span class="meta"><b>#{image.id}</b><small>{image.width} × {image.height}</small></span>
        </button>
      {/each}
    </section>
  {/if}

  <nav class="bottom-nav" aria-label="Asset page navigation">
    <span>Page <b>{page + 1}</b> / {pages}</span>
    <button aria-label="Previous page" onclick={() => go(-1)} disabled={page === 0}>←</button>
    <button aria-label="Next page" onclick={() => go(1)} disabled={page + 1 >= pages}>→</button>
    {#if tab !== 'bitmap'}
      <form class="page-jump" onsubmit={(event) => { event.preventDefault(); jumpToPage(); }}>
        <label>Go to page<input type="number" min="1" max={pages} placeholder="1–{pages}" bind:value={jumpPage} /></label>
        <button type="submit" disabled={!String(jumpPage).trim()}>Go</button>
      </form>
    {/if}
  </nav>
</main>

{#if selected}
  <div class="modal">
    <div class="modal-panel" role="dialog" aria-modal="true" aria-label={`Asset ${selected.id}`} tabindex="-1">
      <header><div><strong>#{selected.id}</strong><span>{selected.kind} · {selected.width} × {selected.height} · {selected.byteLength.toLocaleString()} decoded bytes</span>{#if selected.kind === 'sprite'}<span>{selected.hotspotX === undefined ? 'No hotspot' : `Hotspot ${selected.hotspotX}, ${selected.hotspotY}`} · format 0x{selected.magic?.toString(16)}</span>{/if}</div><button onclick={() => selected = undefined}>Close</button></header>
      <div class="large-preview"><AssetCanvas image={selected} palette={chosenPalette} /></div>
    </div>
  </div>
{/if}
</div>
