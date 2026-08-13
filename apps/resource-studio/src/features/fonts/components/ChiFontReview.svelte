<script lang="ts">
  import { onMount } from 'svelte';
  import { CHIFONT_MAP, parseChiFont, type ChiGlyph } from '@doraemon-monopoly/dubbing-core';
  import ChiGlyphCanvas from './ChiGlyphCanvas.svelte';

  const pageSize = 128;
  let glyphs = $state<ChiGlyph[]>([]);
  let query = $state('');
  let page = $state(0);
  let status = $state('Load chifont.dat to inspect its 16×16 Chinese glyph atlas.');
  let error = $state('');

  function character(index: number) {
    return CHIFONT_MAP[index];
  }

  const filtered = $derived(
    glyphs.filter((glyph) => {
      const term = query.trim();
      const lower = term.toLowerCase();
      if (!lower) return true;
      if (String(glyph.index).includes(lower)) return true;
      const ch = character(glyph.index);
      return Boolean(ch) && ch.includes(term);
    })
  );
  const pages = $derived(Math.max(1, Math.ceil(filtered.length / pageSize)));
  const visible = $derived(filtered.slice(page * pageSize, (page + 1) * pageSize));
  const mappedCount = $derived(glyphs.filter((glyph) => character(glyph.index)).length);

  function load(bytes: Uint8Array, name: string) {
    error = '';
    try {
      const parsed = parseChiFont(bytes);
      glyphs = parsed;
      page = 0;
      query = '';
      status = `Decoded ${parsed.length.toLocaleString()} 16×16 glyphs from ${name} (${mappedCount.toLocaleString()} mapped to Chinese characters).`;
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
      status = 'Loading failed.';
    }
  }

  onMount(async () => {
    try {
      const response = await fetch('/game/chifont.dat');
      if (response.ok) load(new Uint8Array(await response.arrayBuffer()), 'chifont.dat');
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

  function go(delta: number) {
    page = Math.max(0, Math.min(pages - 1, page + delta));
  }

  function hasInk(glyph: ChiGlyph) {
    return glyph.pixels.some(Boolean);
  }
</script>

<div class="chifont-review">
  <section class="font-resource-actions">
    <label class="load-button"
      >Load chifont.dat<input
        type="file"
        accept=".dat,application/octet-stream"
        onchange={fileInput}
      /></label
    >
  </section>
  <p class="status">{status}</p>
  {#if error}<p class="error">{error}</p>{/if}

  {#if !glyphs.length}
    <section
      class="drop-zone"
      role="group"
      aria-label="Load chifont.dat"
      ondragover={(event) => event.preventDefault()}
      ondrop={drop}
    >
      <strong>Load your own chifont.dat</strong><span
        >Drop the file here or use the button above. The atlas holds 747 headerless 16×16 one-bit glyphs (32
        bytes each) addressed by the two-byte Chinese glyph IDs in strings.dat.</span
      >
    </section>
  {:else}
    <p class="subtle">
      Each cell is glyph <code>index</code> of the atlas. Glyph IDs referenced by strings.dat map to these records.
      Slots with no mapped character are empty but may still be referenced at runtime.
    </p>
    <section class="review-toolbar">
      <label
        >Find glyph or character<input
          type="search"
          placeholder="e.g. 141 or 好"
          bind:value={query}
          oninput={() => (page = 0)}
        /></label
      >
    </section>
    <p class="review-count">
      Showing {visible.length} of {filtered.length.toLocaleString()} glyphs · {mappedCount.toLocaleString()}
      mapped characters
    </p>
    <section class="review-grid">
      {#each visible as glyph (glyph.index)}
        <article class:blank={!hasInk(glyph)}>
          <div class="glyph-preview"><ChiGlyphCanvas {glyph} scale={3} /></div>
          <strong>{character(glyph.index) ?? 'reserved'}</strong><small
            >#{glyph.index.toString().padStart(3, '0')} · ID {glyph.index}</small
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
  .chifont-review {
    display: grid;
    gap: 4px;
  }
  .review-toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 8px 0;
  }
  .review-toolbar label {
    color: #516477;
    font-size: 0.82rem;
  }
  .review-toolbar input {
    margin-left: 6px;
    padding: 7px 9px;
    border: 1px solid #a8c9e7;
    border-radius: 8px;
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
