<script lang="ts">
  import './asset-viewer.css';
  import AssetCanvas from './AssetCanvas.svelte';
  import { diagnosticPalette, encodeSprite, readBitmaps, readSprites, type IndexedImage, type Palette } from './lib/asset-formats';
  import { extractGameOneArchive, rebuildGameOneArchive, type GameOneArchiveEntry } from './lib/formats';
  import { decodeIndexedPng, encodeIndexedPng, transparencyIndex } from './lib/indexed-png';
  import { storedZip } from './lib/stored-zip';

  const pageSize = 96;
  let bitmaps: IndexedImage[] = $state([]);
  let sprites: IndexedImage[] = $state([]);
  let bitmapName = $state('');
  let spriteName = $state('');
  let spriteArchiveBytes: Uint8Array | undefined = $state();
  let spriteEntries: GameOneArchiveEntry[] = $state([]);
  let originalSprites = $state(new Map<string, IndexedImage>());
  let modified = $state(new Set<string>());
  let tab: 'bitmap' | 'sprite' = $state('bitmap');
  let page = $state(0);
  let jumpPage = $state('');
  let exportFrom = $state('0');
  let exportTo = $state('95');
  let query = $state('');
  let paletteId = $state('053');
  let status = $state('Load the bundled bitmap and sprite archives when needed.');
  let error = $state('');
  let busy = $state(false);
  let dragging = $state(false);
  let selected: IndexedImage | undefined = $state();

  let current = $derived(tab === 'bitmap' ? bitmaps : sprites);
  let filtered = $derived(current.filter((image) => !query.trim() || image.id.includes(query.trim()) || `${image.width}x${image.height}`.includes(query.trim().toLowerCase())));
  let pages = $derived(Math.max(1, Math.ceil(filtered.length / pageSize)));
  let visible = $derived(filtered.slice(page * pageSize, (page + 1) * pageSize));
  let chosenBitmap = $derived(bitmaps.find((image) => image.id === paletteId.padStart(3, '0') && image.palette));
  let chosenPalette: Palette = $derived(chosenBitmap?.palette ?? diagnosticPalette());

  function resetView(nextTab: 'bitmap' | 'sprite') { tab = nextTab; page = 0; query = ''; }

  async function loadBytes(bytes: Uint8Array, fileName: string) {
    error = '';
    status = `Reading ${fileName}…`;
    await new Promise((resolve) => setTimeout(resolve, 0));
    try {
      const name = fileName.toLowerCase();
      if (name.includes('sprite')) {
        const result = readSprites(bytes);
        sprites = result.images;
        spriteArchiveBytes = bytes;
        spriteEntries = result.entries;
        originalSprites = new Map(result.images.map((image) => [image.id, image]));
        modified = new Set();
        spriteName = fileName;
        resetView('sprite');
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
          originalSprites = new Map(spriteResult.images.map((image) => [image.id, image])); modified = new Set(); spriteName = fileName; resetView('sprite');
          status = `Detected ${spriteResult.images.length.toLocaleString()} sprites.`;
        }
      }
    } catch (cause) { error = cause instanceof Error ? cause.message : String(cause); status = 'Loading failed.'; }
  }

  async function loadBundled(url: string, name: string) {
    error = '';
    status = `Loading ${name}…`;
    try {
      const response = await fetch(url);
      if (!response.ok) throw new Error(`${name} returned HTTP ${response.status}.`);
      await loadBytes(new Uint8Array(await response.arrayBuffer()), name);
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

  function range() {
    const from = Number(exportFrom), to = Number(exportTo);
    if (!Number.isInteger(from) || !Number.isInteger(to) || from < 0 || to > 13806 || from > to) {
      throw new Error('Sprite range must use whole IDs from 0 through 13806, with From no greater than To.');
    }
    return { from, to };
  }

  async function exportIndexedRange() {
    error = '';
    try {
      if (!chosenBitmap?.palette) throw new Error('Load bitmaps.dat and select a valid bitmap palette before exporting sprites.');
      const { from, to } = range();
      const selectedSprites = sprites.filter((image) => { const id = Number(image.id); return id >= from && id <= to; });
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
      download(storedZip(entries), `sprites-${from}-${to}.zip`);
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
      return { file, id: Number.isInteger(numeric) && numeric >= 0 && numeric <= 13806 ? String(numeric).padStart(3, '0') : '' };
    });
    const counts = new Map<string, number>();
    for (const item of named) if (item.id) counts.set(item.id, (counts.get(item.id) ?? 0) + 1);
    const rejected: string[] = [];
    let applied = 0;
    busy = true;
    try {
      for (const item of named) {
        if (!item.id) { rejected.push(`${item.file.name}: filename must be a sprite number followed by .png`); continue; }
        if ((counts.get(item.id) ?? 0) > 1) { rejected.push(`${item.file.name}: duplicate sprite ID ${Number(item.id)}`); continue; }
        const original = originalSprites.get(item.id);
        if (!original) { rejected.push(`${item.file.name}: ID is not a decoded sprite`); continue; }
        try {
          const png = await decodeIndexedPng(new Uint8Array(await item.file.arrayBuffer()));
          if (png.width !== original.width || png.height !== original.height) throw new Error(`expected ${original.width}×${original.height}, got ${png.width}×${png.height}`);
          const replacement: IndexedImage = { ...original, pixels: png.pixels, alpha: png.alpha };
          sprites = sprites.map((image) => image.id === item.id ? replacement : image);
          modified = new Set([...modified, item.id]);
          applied += 1;
        } catch (cause) { rejected.push(`${item.file.name}: ${cause instanceof Error ? cause.message : String(cause)}`); }
      }
      status = `Applied ${applied} indexed sprite replacement${applied === 1 ? '' : 's'}${rejected.length ? `; rejected ${rejected.length}` : ''}.`;
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
      if (!spriteArchiveBytes || !spriteEntries.length) throw new Error('Load the original Sprite1.dat first.');
      if (!modified.size) throw new Error('No sprite replacements have been imported.');
      busy = true; status = `Rebuilding ${modified.size} modified sprite records…`;
      await new Promise((resolve) => setTimeout(resolve, 0));
      const current = new Map(sprites.map((image) => [image.id, image]));
      const replacements = new Map<string, Uint8Array>();
      for (const id of modified) replacements.set(id, encodeSprite(current.get(id)!));
      const rebuilt = rebuildGameOneArchive(spriteArchiveBytes, replacements);
      const rebuiltArchive = extractGameOneArchive(rebuilt);
      const rebuiltEntries = new Map(rebuiltArchive.map((entry) => [entry.id, entry]));
      if (rebuiltArchive.length !== spriteEntries.length) throw new Error('Rebuilt archive changed the record count.');
      for (const original of spriteEntries) {
        if (!modified.has(original.id) && !sameBytes(original.packed, rebuiltEntries.get(original.id)?.packed ?? new Uint8Array())) {
          throw new Error(`Untouched record ${original.id} changed unexpectedly.`);
        }
      }
      const verified = new Map(readSprites(rebuilt).images.map((image) => [image.id, image]));
      for (const id of modified) {
        const expected = current.get(id)!, actual = verified.get(id);
        if (!actual || actual.width !== expected.width || actual.height !== expected.height || actual.magic !== expected.magic || actual.hotspotX !== expected.hotspotX || actual.hotspotY !== expected.hotspotY || !sameBytes(actual.pixels, expected.pixels) || !sameBytes(actual.alpha!, expected.alpha!)) {
          throw new Error(`Sprite verification failed for ${id}.`);
        }
      }
      download(new Blob([rebuilt], { type: 'application/octet-stream' }), 'Sprite1-modified.dat');
      status = `Exported and verified Sprite1-modified.dat with ${modified.size} modified sprites.`;
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
  </section>

  <section class="load-panel" aria-label="Asset loaders">
    <strong>Original game assets</strong>
    <span>Load the original archives bundled with this project; nothing is uploaded or modified.</span>
    <div><button type="button" onclick={() => loadBundled('/bitmaps.dat', 'bitmaps.dat')}>Load original bitmaps.dat</button><button type="button" onclick={() => loadBundled('/Sprite1.dat', 'Sprite1.dat')}>Load original Sprite1.dat</button></div>
  </section>

  {#if error}<p class="error">{error}</p>{/if}
  <p class="status">{status}</p>

  <nav class="tabs" aria-label="Asset type">
    <button class:active={tab === 'bitmap'} onclick={() => resetView('bitmap')}>Bitmaps <b>{bitmaps.length.toLocaleString()}</b></button>
    <button class:active={tab === 'sprite'} onclick={() => resetView('sprite')}>Sprites <b>{sprites.length.toLocaleString()}</b></button>
  </nav>

  <section class="toolbar">
    <label>Find entry or size<input type="search" placeholder="e.g. 053 or 640x480" bind:value={query} oninput={() => page = 0} /></label>
    {#if tab === 'sprite'}<label>Palette bitmap ID<input class="palette-id" inputmode="numeric" bind:value={paletteId} /></label><span class="palette-state">{bitmaps.some((image) => image.id === paletteId.padStart(3, '0')) ? `Using bitmap ${paletteId.padStart(3, '0')}` : 'Using diagnostic colors'}</span>{/if}
  </section>

  {#if tab === 'sprite' && sprites.length}
    <section class="sprite-editor" aria-label="Indexed sprite editor">
      <div class="sprite-export">
        <div><strong>Export indexed PNGs</strong><span>A real bitmap palette is required. The ID range is inclusive.</span></div>
        <label>From ID<input type="number" min="0" max="13806" bind:value={exportFrom} /></label>
        <label>To ID<input type="number" min="0" max="13806" bind:value={exportTo} /></label>
        <button type="button" disabled={busy || !chosenBitmap?.palette} onclick={exportIndexedRange}>Export PNG ZIP</button>
      </div>
      <div class:dragging class="sprite-import" role="group" aria-label="Indexed sprite PNG import" ondragover={(event) => { event.preventDefault(); dragging = true; }} ondragleave={() => dragging = false} ondrop={dropPngs}>
        <div><strong>Import Aseprite replacements</strong><span>Drop numbered indexed PNGs here, or choose several files. RGB/RGBA and resized images are rejected.</span></div>
        <label class="file-button">Choose PNGs<input type="file" accept="image/png,.png" multiple onchange={importInput} /></label>
      </div>
      <div class="aseprite-note"><strong>Aseprite safety</strong><span>Keep <b>Indexed</b> color mode, the same dimensions, palette order, and transparent slot. Draw with palette entries, erase for transparency, and save directly under the original numbered filename. Reordering palette colors changes the game’s pixel indices even when the image still looks correct.</span></div>
      <div class="sprite-save"><span><b>{modified.size}</b> modified sprite{modified.size === 1 ? '' : 's'}. Palette RGB is preview-only; imported pixel indices are used exactly.</span><button type="button" class="primary" disabled={busy || !modified.size} onclick={exportModifiedSprites}>Export modified Sprite1.dat</button></div>
    </section>
  {/if}

  {#if !current.length}
    <section class="empty">Load {tab === 'bitmap' ? 'bitmaps.dat' : 'Sprite1.dat'} to inspect it here.</section>
  {:else}
    <p class="count">Showing {visible.length} of {filtered.length.toLocaleString()} decoded {tab === 'bitmap' ? 'bitmaps' : 'sprites'} · {tab === 'bitmap' ? bitmapName : spriteName}</p>
    <section class="grid">
      {#each visible as image (image.id)}
        <button class:modified={modified.has(image.id)} class="asset" onclick={() => selected = image} title={`Open ${image.id}`}>
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
    {#if tab === 'sprite'}
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
      <header><div><strong>#{selected.id}</strong><span>{selected.kind} · {selected.width} × {selected.height} · {selected.byteLength.toLocaleString()} decoded bytes</span>{#if selected.kind === 'sprite'}<span>Hotspot {selected.hotspotX}, {selected.hotspotY} · format 0x{selected.magic?.toString(16)}</span>{/if}</div><button onclick={() => selected = undefined}>Close</button></header>
      <div class="large-preview"><AssetCanvas image={selected} palette={chosenPalette} /></div>
    </div>
  </div>
{/if}
</div>
