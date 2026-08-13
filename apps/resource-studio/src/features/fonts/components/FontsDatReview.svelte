<script lang="ts">
  import { onMount } from 'svelte';
  import { readSprites, type IndexedImage } from '../../../lib/asset-formats';
  import IndexedCanvas from '../../graphics/components/IndexedCanvas.svelte';
  import { loadCanonicalPalettes, type FavoritePalette } from '../../graphics/palette-favorites';

  const pageSize = 144;
  const bankSize = 128;
  const bankCount = 20;
  let images = $state<IndexedImage[]>([]);
  let query = $state('');
  let page = $state(0);
  let bank = $state<number | 'all'>('all');
  let paletteId = $state('001');
  let favoritePalettes = $state<FavoritePalette[]>([]);
  let status = $state('Load Fonts.dat to inspect its glyph records.');
  let error = $state('');

  const chosenPalette = $derived(favoritePalettes.find((favorite) => favorite.id === paletteId));

  const indexed = $derived(
    images.map((image, index) => ({
      image,
      leaf: index,
      bank: Math.floor(index / bankSize),
      slot: index % bankSize
    }))
  );

  const bankVisible = $derived.by(() => {
    const counts: number[] = new Array(bankCount).fill(0);
    for (const item of indexed) if (item.image.alpha?.some((alpha) => alpha !== 0)) counts[item.bank] += 1;
    return counts;
  });

  const grouped = $derived(bank === 'all' ? indexed : indexed.filter((item) => item.bank === bank));
  const filtered = $derived(
    grouped.filter(
      (item) =>
        !query.trim() ||
        String(item.leaf).includes(query.trim()) ||
        `${item.image.width}x${item.image.height}`.includes(query.trim().toLowerCase())
    )
  );
  const pages = $derived(Math.max(1, Math.ceil(filtered.length / pageSize)));
  const visible = $derived(filtered.slice(page * pageSize, (page + 1) * pageSize));
  const visibleGlyphs = $derived(
    indexed.filter((item) => item.image.alpha?.some((alpha) => alpha !== 0)).length
  );

  function load(bytes: Uint8Array, name: string) {
    error = '';
    try {
      const result = readSprites(bytes);
      if (!result.images.length)
        throw new Error('No 0x8002 scanline glyph records were found in this archive.');
      images = result.images;
      page = 0;
      query = '';
      bank = 'all';
      status = `Decoded ${result.images.length.toLocaleString()} glyph leaves from ${name} — ${bankCount} banks of ${bankSize} (${visibleGlyphs.toLocaleString()} with visible pixels).`;
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
      status = 'Loading failed.';
    }
  }

  onMount(async () => {
    void loadCanonicalPalettes().then((palettes) => (favoritePalettes = palettes));
    try {
      const response = await fetch('/game/Fonts.dat');
      if (response.ok) load(new Uint8Array(await response.arrayBuffer()), 'Fonts.dat');
    } catch {
      /* Optional local development file. */
    }
  });

  async function fileInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    if (input.files?.[0]) load(new Uint8Array(await input.files[0].arrayBuffer()), input.files[0].name);
    input.value = '';
  }

  function drop(event: DragEvent) {
    event.preventDefault();
    if (!event.dataTransfer?.files.length) return;
    const file = Array.from(event.dataTransfer.files)[0];
    if (file) void file.arrayBuffer().then((bytes) => load(new Uint8Array(bytes), file.name));
  }

  function selectBank(next: number | 'all') {
    bank = next;
    page = 0;
  }

  function go(delta: number) {
    page = Math.max(0, Math.min(pages - 1, page + delta));
  }

  function scaleFor(image: IndexedImage) {
    const largest = Math.max(image.width, image.height);
    return largest < 48 ? Math.min(3, 48 / largest) : 1;
  }

  function hasPixels(item: { image: IndexedImage }) {
    return !!item.image.alpha?.some((alpha) => alpha !== 0);
  }

  function paletteMeaning() {
    return chosenPalette
      ? `#${chosenPalette.id} ${chosenPalette.label} — ${chosenPalette.meaning}`
      : favoritePalettes.length
        ? ''
        : 'Load the original bitmaps.dat to obtain the three canonical review palettes.';
  }
</script>

<div class="fonts-dat-review">
  <section class="font-resource-actions">
    <label class="load-button"
      >Load Fonts.dat<input type="file" accept=".dat,application/octet-stream" onchange={fileInput} /></label
    >
  </section>
  <p class="status">{status}</p>
  {#if error}<p class="error">{error}</p>{/if}

  {#if !images.length}
    <section
      class="drop-zone"
      role="group"
      aria-label="Load Fonts.dat"
      ondragover={(event) => event.preventDefault()}
      ondrop={drop}
    >
      <strong>Load your own Fonts.dat</strong><span
        >Drop the file here or use the button above. The original file is a GameOne archive of 2,560 scanline
        glyph records (0x8002) used for decorative font artwork.</span
      >
    </section>
  {:else}
    <p class="subtle">
      The leaves are styled proportional glyphs — ornamental Latin capitals, circular monograms and
      speech-bubble shapes — not the Chinese glyph set. Chinese characters come from
      <code>chifont.dat</code>, reviewed in the neighbouring tab. The records repeat in groups of 128; the
      <em>bank</em> chips below jump between those groups.
    </p>
    <section class="review-toolbar">
      <label
        >Find glyph or size<input
          type="search"
          placeholder="e.g. 1200 or 18x32"
          bind:value={query}
          oninput={() => (page = 0)}
        /></label
      >
      <label
        >Colour palette
        <select bind:value={paletteId} disabled={!favoritePalettes.length}>
          {#if !favoritePalettes.length}<option value="001">Load the original bitmaps.dat first</option>{/if}
          {#each favoritePalettes as palette (palette.id)}
            <option value={palette.id}>#{palette.id} · {palette.label}</option>
          {/each}
        </select>
      </label>
    </section>
    <p class="palette-status">{paletteMeaning()}</p>
    <section class="bank-chips" aria-label="Glyph banks">
      <button class:active={bank === 'all'} onclick={() => selectBank('all')}
        >All·{visibleGlyphs.toLocaleString()}</button
      >
      {#each Array.from({ length: bankCount }, (_, bankIndex) => bankIndex) as bankIndex (bankIndex)}
        <button class:active={bank === bankIndex} onclick={() => selectBank(bankIndex)}
          >B{bankIndex}·{bankVisible[bankIndex] ?? 0}</button
        >
      {/each}
    </section>
    <p class="review-count">
      Showing {visible.length} of {filtered.length.toLocaleString()} leaves
      {bank === 'all' ? '' : ` in bank ${bank} (slots ${bank * bankSize}–${bank * bankSize + bankSize - 1})`} ·
      {visibleGlyphs.toLocaleString()} with visible pixels
    </p>
    <section class="review-grid">
      {#each visible as { image, leaf, bank: leafBank, slot } (leaf)}
        <article class:blank={!hasPixels({ image: image })}>
          <div class="glyph-preview">
            <IndexedCanvas {image} palette={chosenPalette?.palette} scale={scaleFor(image)} fitVisible />
          </div>
          <strong>#{leaf}</strong><small>bank {leafBank}·slot {slot} · {image.width} × {image.height}px</small
          >
        </article>
      {/each}
    </section>
    <nav class="bottom-nav" aria-label="Glyph page navigation">
      <span>Page <b>{page + 1}</b> / {pages}</span>
      <button aria-label="Previous page" onclick={() => go(-1)} disabled={page === 0}>←</button>
      <button aria-label="Next page" onclick={() => go(1)} disabled={page + 1 >= pages}>→</button>
    </nav>
  {/if}
</div>

<style>
  .fonts-dat-review {
    display: grid;
    gap: 4px;
  }
  .review-toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 8px 0;
    flex-wrap: wrap;
  }
  .review-toolbar label {
    color: #516477;
    font-size: 0.82rem;
  }
  .review-toolbar input,
  .review-toolbar select {
    margin-left: 6px;
    padding: 7px 9px;
    border: 1px solid #a8c9e7;
    border-radius: 8px;
  }
  .palette-status {
    margin: 2px 0 8px;
    color: #667789;
    font-size: 0.8rem;
  }
  .bank-chips {
    display: flex;
    gap: 6px;
    overflow-x: auto;
    padding-bottom: 8px;
  }
  .bank-chips button {
    flex: 0 0 auto;
    padding: 4px 9px;
    border: 1px solid #c9d6e4;
    border-radius: 999px;
    background: #fff;
    color: #516477;
    font-size: 0.74rem;
  }
  .bank-chips button.active {
    border-color: #2b6cb0;
    background: #2b6cb0;
    color: #fff;
  }
  .review-count {
    color: #667789;
    font-size: 0.8rem;
  }
  .review-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
    gap: 8px;
  }
  .review-grid article {
    display: grid;
    justify-items: center;
    gap: 4px;
    min-height: 126px;
    padding: 8px 5px;
    border: 1px solid #d6dee7;
    border-radius: 9px;
    background: #fff;
  }
  .review-grid article.blank {
    opacity: 0.45;
  }
  .review-grid article strong {
    max-width: 100%;
    overflow: hidden;
    color: #17212b;
    font-size: 0.76rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .review-grid article small {
    color: #667789;
    font-size: 0.68rem;
  }
  .bottom-nav {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 14px;
  }
  .bottom-nav > span {
    margin-right: auto;
    color: #516477;
    font-size: 0.82rem;
  }
  .bottom-nav > span b {
    color: #17212b;
  }
  .bottom-nav button {
    border-radius: 8px;
    padding: 6px 11px;
  }
</style>
