<script lang="ts">
  import './asset-viewer.css';
  import AssetCanvas from './AssetCanvas.svelte';
  import { diagnosticPalette, readBitmaps, readSprites, type IndexedImage, type Palette } from './lib/asset-formats';

  const pageSize = 96;
  let bitmaps: IndexedImage[] = $state([]);
  let sprites: IndexedImage[] = $state([]);
  let bitmapName = $state('');
  let spriteName = $state('');
  let tab: 'bitmap' | 'sprite' = $state('bitmap');
  let page = $state(0);
  let query = $state('');
  let paletteId = $state('053');
  let status = $state('Drop either file here. Load both for correctly colored sprites.');
  let error = $state('');
  let dragging = $state(false);
  let selected: IndexedImage | undefined = $state();

  let current = $derived(tab === 'bitmap' ? bitmaps : sprites);
  let filtered = $derived(current.filter((image) => !query.trim() || image.id.includes(query.trim()) || `${image.width}x${image.height}`.includes(query.trim().toLowerCase())));
  let pages = $derived(Math.max(1, Math.ceil(filtered.length / pageSize)));
  let visible = $derived(filtered.slice(page * pageSize, (page + 1) * pageSize));
  let chosenPalette: Palette = $derived(bitmaps.find((image) => image.id === paletteId.padStart(3, '0'))?.palette ?? diagnosticPalette());

  function resetView(nextTab: 'bitmap' | 'sprite') { tab = nextTab; page = 0; query = ''; }

  async function load(file: File) {
    error = '';
    status = `Reading ${file.name}…`;
    await new Promise((resolve) => setTimeout(resolve, 0));
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const name = file.name.toLowerCase();
      if (name.includes('sprite')) {
        const result = readSprites(bytes);
        sprites = result.images;
        spriteName = file.name;
        resetView('sprite');
        status = `Decoded ${sprites.length.toLocaleString()} sprites from ${file.name}.`;
      } else if (name.includes('bitmap')) {
        const result = readBitmaps(bytes);
        bitmaps = result.images;
        bitmapName = file.name;
        resetView('bitmap');
        status = `Decoded ${bitmaps.length.toLocaleString()} PCX bitmaps from ${file.name}.`;
      } else {
        const bitmapResult = readBitmaps(bytes);
        if (bitmapResult.images.length) {
          bitmaps = bitmapResult.images; bitmapName = file.name; resetView('bitmap');
          status = `Detected ${bitmapResult.images.length.toLocaleString()} PCX bitmaps.`;
        } else {
          const spriteResult = readSprites(bytes);
          if (!spriteResult.images.length) throw new Error('This is a GameOne archive, but no supported PCX bitmaps or sprites were found.');
          sprites = spriteResult.images; spriteName = file.name; resetView('sprite');
          status = `Detected ${spriteResult.images.length.toLocaleString()} sprites.`;
        }
      }
    } catch (cause) { error = cause instanceof Error ? cause.message : String(cause); status = 'Loading failed.'; }
  }

  async function loadFiles(files: FileList | File[]) { for (const file of Array.from(files)) await load(file); }
  function drop(event: DragEvent) { event.preventDefault(); dragging = false; if (event.dataTransfer?.files.length) loadFiles(event.dataTransfer.files); }
  function go(delta: number) { page = Math.max(0, Math.min(pages - 1, page + delta)); }
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

  <section class:dragging class="drop" role="group" aria-label="Asset file drop area" ondragover={(event) => { event.preventDefault(); dragging = true; }} ondragleave={() => dragging = false} ondrop={drop}>
    <strong>Drop bitmaps.dat and/or Sprite1.dat</strong>
    <span>They remain in memory only. Nothing is uploaded or modified.</span>
    <div><label>Load bitmaps.dat<input type="file" onchange={(event) => loadFiles(event.currentTarget.files!)} /></label><label>Load Sprite1.dat<input type="file" onchange={(event) => loadFiles(event.currentTarget.files!)} /></label></div>
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
    <div class="pager"><button onclick={() => go(-1)} disabled={page === 0}>←</button><span>Page {page + 1} / {pages}</span><button onclick={() => go(1)} disabled={page + 1 >= pages}>→</button></div>
  </section>

  {#if !current.length}
    <section class="empty">Load {tab === 'bitmap' ? 'bitmaps.dat' : 'Sprite1.dat'} to inspect it here.</section>
  {:else}
    <p class="count">Showing {visible.length} of {filtered.length.toLocaleString()} decoded {tab === 'bitmap' ? 'bitmaps' : 'sprites'} · {tab === 'bitmap' ? bitmapName : spriteName}</p>
    <section class="grid">
      {#each visible as image (image.id)}
        <button class="asset" onclick={() => selected = image} title={`Open ${image.id}`}>
          <span class="preview"><AssetCanvas {image} palette={chosenPalette} /></span>
          <span class="meta"><b>#{image.id}</b><small>{image.width} × {image.height}</small></span>
        </button>
      {/each}
    </section>
  {/if}
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
