<script lang="ts">
  import { onMount } from 'svelte';
  import { binaryBlob, downloadBlob } from '../../lib/browser-download';
  import StudioHeader from '../../lib/components/StudioHeader.svelte';
  import { parseSysFont, rebuildSysFont, type SysFont, type SysGlyph } from '@doraemon-monopoly/dubbing-core';
  import {
    extendSysFont,
    VIETNAMESE_CHARACTERS,
    VIETNAMESE_SLOTS_PER_VARIANT,
    EXTENDED_SYSFONT_GLYPHS,
    vietnameseBytes,
    vietnameseGlyphIndex
  } from '@doraemon-monopoly/dubbing-core';
  import { storedZip } from '@doraemon-monopoly/dubbing-core';
  import FontGlyph from './components/FontGlyph.svelte';
  import FontsDatReview from './components/FontsDatReview.svelte';
  import ChiFontReview from './components/ChiFontReview.svelte';

  type FontMode = 'sysfont' | 'fonts-dat' | 'chifont';
  let mode = $state<FontMode>('sysfont');
  let font: SysFont | undefined = $state();
  let family = $state<'original' | 'vietnamese'>('original');
  let variant = $state(0);
  let error = $state('');
  let status = $state('Load sysfont.dat to inspect or edit fonts.');
  let dragging = $state(false);
  let modified = $state(new Set<number>());

  let hasVietnamese = $derived(font && font.count >= EXTENDED_SYSFONT_GLYPHS);

  async function loadFont(file: Blob, name: string) {
    error = '';
    try {
      const parsed = parseSysFont(new Uint8Array(await file.arrayBuffer()));
      console.log(parsed.count, EXTENDED_SYSFONT_GLYPHS);
      const extended = parsed.count >= EXTENDED_SYSFONT_GLYPHS;
      font = extended ? extendSysFont(parsed) : parsed;
      modified = new Set();
      family = 'original';
      variant = 0;
      status = extended
        ? `${name} · ${font.count} glyphs · five original variants · five Vietnamese banks`
        : `${name} · ${font.count} glyphs`;
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
      status = 'Loading failed.';
    }
  }

  onMount(async () => {
    try {
      const response = await fetch('/game/sysfont.dat');
      if (response.ok) await loadFont(await response.blob(), 'sysfont.dat');
    } catch {
      /* Optional local development file. */
    }
  });

  async function fontInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    if (input.files?.[0]) await loadFont(input.files[0], input.files[0].name);
    input.value = '';
  }

  function label(code: number) {
    if (code === 0) return 'NUL';
    if (code === 9) return 'TAB';
    if (code === 10) return 'LF';
    if (code === 13) return 'CR';
    if (code === 32) return 'SPACE';
    return code >= 33 && code < 127
      ? JSON.stringify(String.fromCharCode(code))
      : `0x${code.toString(16).padStart(2, '0')}`;
  }

  function pngForGlyph(glyph: SysGlyph) {
    const canvas = document.createElement('canvas');
    canvas.width = glyph.width;
    canvas.height = glyph.height;
    const context = canvas.getContext('2d')!;
    const image = context.createImageData(glyph.width, glyph.height);
    for (let index = 0; index < glyph.pixels.length; index += 1) {
      const pixel = index * 4;
      const visible = glyph.pixels[index] === 0;
      image.data[pixel] = 0;
      image.data[pixel + 1] = 0;
      image.data[pixel + 2] = 0;
      image.data[pixel + 3] = visible ? 255 : 0;
    }
    context.putImageData(image, 0, 0);
    return new Promise<Blob>((resolve, reject) =>
      canvas.toBlob(
        (blob) => (blob ? resolve(blob) : reject(new Error('Could not encode PNG.'))),
        'image/png'
      )
    );
  }

  function visibleStart() {
    return family === 'original' ? variant * 128 : vietnameseGlyphIndex(variant, 0);
  }

  function visibleCount() {
    return family === 'original' ? 128 : VIETNAMESE_SLOTS_PER_VARIANT;
  }

  function vietnameseCodeLabel(slot: number) {
    const character = VIETNAMESE_CHARACTERS[slot];
    if (!character) return `reserved slot ${slot}`;
    const bytes = vietnameseBytes(character)!;
    return `${bytes.map((byte) => byte.toString(16).padStart(2, '0').toUpperCase()).join(' ')} · slot ${slot}`;
  }

  async function exportVariant() {
    if (!font) return;
    const start = visibleStart();
    status = `Encoding ${family} variant ${variant} PNGs…`;
    try {
      const entries = await Promise.all(
        font.glyphs.slice(start, start + visibleCount()).map(async (glyph, index) => ({
          name: `${start + index}.png`,
          bytes: new Uint8Array(await (await pngForGlyph(glyph)).arrayBuffer())
        }))
      );
      downloadBlob(storedZip(entries), `sysfont-${family}-variant-${variant}.zip`);
      status = `Exported ${family} variant ${variant}: ${visibleCount()} numbered transparent PNGs.`;
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    }
  }

  async function importImages(files: FileList | File[]) {
    if (!font) return;
    error = '';
    let applied = 0;
    const rejected: string[] = [];
    for (const file of Array.from(files)) {
      const match = /^(\d+)\.png$/i.exec(file.name);
      const index = match ? Number(match[1]) : NaN;
      if (!Number.isInteger(index) || index < 0 || index >= font.count) {
        rejected.push(file.name);
        continue;
      }
      try {
        const bitmap = await createImageBitmap(file);
        if (!bitmap.width || !bitmap.height || bitmap.width > 96 || bitmap.height > 96)
          throw new Error('dimensions must be 1–96px');
        const canvas = document.createElement('canvas');
        canvas.width = bitmap.width;
        canvas.height = bitmap.height;
        const context = canvas.getContext('2d', { willReadFrequently: true })!;
        context.drawImage(bitmap, 0, 0);
        const rgba = context.getImageData(0, 0, bitmap.width, bitmap.height).data;
        const pixels = new Uint8Array(bitmap.width * bitmap.height);
        for (let pixel = 0; pixel < pixels.length; pixel += 1) pixels[pixel] = rgba[pixel * 4 + 3] ? 0 : 255;
        bitmap.close();
        const glyph = { width: canvas.width, height: canvas.height, pixels };
        const glyphs: SysGlyph[] = [...font.glyphs];
        glyphs[index] = glyph;
        font = { ...font, glyphs };
        modified = new Set([...modified, index]);
        applied += 1;
      } catch (cause) {
        rejected.push(`${file.name} (${cause instanceof Error ? cause.message : String(cause)})`);
      }
    }
    status = `Applied ${applied} PNG glyph replacement${applied === 1 ? '' : 's'}${rejected.length ? `; skipped ${rejected.length}` : ''}.`;
    if (rejected.length) error = `Skipped: ${rejected.join(', ')}`;
  }

  function drop(event: DragEvent) {
    event.preventDefault();
    if (mode !== 'sysfont') return;
    dragging = false;
    if (!event.dataTransfer?.files.length) return;
    const files = Array.from(event.dataTransfer.files);
    const dataFile = files.find((file) => file.name.toLowerCase() === 'sysfont.dat');
    if (dataFile) void loadFont(dataFile, dataFile.name);
    else if (font) void importImages(files);
    else error = 'Load sysfont.dat before importing numbered PNG glyphs.';
  }
  function exportFont() {
    if (!font) return;
    try {
      downloadBlob(binaryBlob(rebuildSysFont(font)), 'sysfont-modified.dat');
      status = `Exported and verified sysfont-modified.dat (${modified.size} changed glyphs).`;
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    }
  }
</script>

<svelte:window ondragover={(event) => event.preventDefault()} ondrop={drop} />
<main class="font-page">
  <StudioHeader
    title="Font studio"
    description={`Inspect the game's font resources.${hasVietnamese ? ' Five proportional Vietnamese CC/CD banks are available.' : ''}`}
    active="fonts"
  />
  <nav class="font-tabs" aria-label="Font resource">
    <button class:active={mode === 'sysfont'} onclick={() => (mode = 'sysfont')}
      >Sysfont editor <small>edit sysfont.dat</small></button
    >
    <button class:active={mode === 'fonts-dat'} onclick={() => (mode = 'fonts-dat')}
      >Fonts.dat glyphs <small>review 0x8002 leaves</small></button
    >
    <button class:active={mode === 'chifont'} onclick={() => (mode = 'chifont')}
      >Chinese atlas <small>review chifont.dat</small></button
    >
  </nav>
  {#if mode === 'fonts-dat'}
    <FontsDatReview />
  {:else if mode === 'chifont'}
    <ChiFontReview />
  {:else}
    <section class="resource-actions font-resource-actions" aria-label="Font file">
      <label class="load-button"
        >Load sysfont.dat<input
          type="file"
          accept=".dat,application/octet-stream"
          onchange={fontInput}
        /></label
      >
    </section>
    <p class="status">{status}</p>
    {#if error}<p class="error">{error}</p>{/if}
    {#if !font}
      <section
        class="drop-zone"
        role="group"
        aria-label="Load sysfont"
        ondragover={(event) => event.preventDefault()}
        ondrop={drop}
      >
        <strong>Load your own sysfont.dat</strong><span
          >Drop the file here or use the button above. No game font is bundled with the Studio.</span
        >
      </section>
    {/if}
    {#if font}
      <nav class="font-tabs" aria-label="Font family">
        <button class:active={family === 'original'} onclick={() => (family = 'original')}
          >Original sysfont</button
        >
        {#if hasVietnamese}
          <button class:active={family === 'vietnamese'} onclick={() => (family = 'vietnamese')}
            >Vietnamese CC/CD</button
          >
        {/if}
      </nav>
      <nav class="font-tabs" aria-label="Font variant">
        {#each Array.from({ length: 5 }, (_, index) => index) as index (index)}<button
            class:active={variant === index}
            onclick={() => (variant = index)}
            >Variant {index}
            <small
              >{family === 'original'
                ? `${index * 128}–${index * 128 + 127}`
                : `${vietnameseGlyphIndex(index, 0)}–${vietnameseGlyphIndex(index, 255)}`}</small
            ></button
          >{/each}
      </nav>
      <section
        class:dragging
        class="font-import"
        role="group"
        aria-label="Sysfont PNG import"
        ondragover={(event) => {
          event.preventDefault();
          dragging = true;
        }}
        ondragleave={() => (dragging = false)}
        ondrop={drop}
      >
        <strong>Drop replacement PNGs here</strong><span
          >Name each image by its absolute glyph index: <code>0.png</code> through <code>1919.png</code>.
          Transparent pixels become background; every non-transparent pixel becomes a drawn black font pixel.</span
        >
      </section>
      <div class="font-actions">
        <button type="button" onclick={exportVariant}>Export variant {variant} PNGs</button><button
          type="button"
          class="primary"
          disabled={!modified.size}
          onclick={exportFont}>Export modified sysfont.dat</button
        ><span>{modified.size} modified glyphs</span>
      </div>
      <p class="subtle font-note">
        Vietnamese slots use <code>640 + variant × 256 + slot</code>. Variant 0 is
        <strong>bootstrap art only</strong> — its glyphs are auto-generated by stamping marks at guessed positions
        and most are incorrect; variants 1–4 are valid transparent placeholders. Fix every Vietnamese glyph via
        PNG replacement (export, edit, re-import) before release; never ship the generated glyphs as-is.
      </p>
      <section class="font-grid">
        {#each font.glyphs.slice(visibleStart(), visibleStart() + visibleCount()) as glyph, index (visibleStart() + index)}<article
            class:modified={modified.has(visibleStart() + index)}
          >
            <div class="glyph-preview"><FontGlyph {glyph} /></div>
            <code>#{(visibleStart() + index).toString().padStart(4, '0')}</code><strong
              >{family === 'original' ? label(index) : VIETNAMESE_CHARACTERS[index] || 'reserved'}</strong
            >{#if family === 'vietnamese'}<small>{vietnameseCodeLabel(index)}</small>{/if}<small
              >{glyph.width} × {glyph.height}px</small
            >
          </article>{/each}
      </section>
    {/if}
  {/if}
</main>
