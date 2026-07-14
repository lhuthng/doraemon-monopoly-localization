<script lang="ts">
  import { onMount } from 'svelte';
  import SysfontGlyph from './SysfontGlyph.svelte';
  import { parseSysFont, rebuildSysFont, type SysFont, type SysGlyph } from './lib/formats';

  let font: SysFont | undefined = $state();
  let variant = $state(0);
  let error = $state('');
  let status = $state('Loading bundled sysfont.dat…');
  let dragging = $state(false);
  let modified = $state(new Set<number>());

  onMount(async () => {
    try {
      const response = await fetch('/sysfont.dat');
      if (!response.ok) throw new Error(`sysfont.dat returned HTTP ${response.status}.`);
      font = parseSysFont(new Uint8Array(await response.arrayBuffer()));
      status = `${font.count} glyphs · ${font.variants} variants · 128 slots per variant`;
    } catch (cause) { error = cause instanceof Error ? cause.message : String(cause); status = 'Loading failed.'; }
  });

  function label(code: number) {
    if (code === 0) return 'NUL'; if (code === 9) return 'TAB'; if (code === 10) return 'LF'; if (code === 13) return 'CR'; if (code === 32) return 'SPACE';
    return code >= 33 && code < 127 ? JSON.stringify(String.fromCharCode(code)) : `0x${code.toString(16).padStart(2, '0')}`;
  }

  function download(blob: Blob, name: string) {
    const url = URL.createObjectURL(blob); const link = document.createElement('a'); link.href = url; link.download = name; link.click();
    window.setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  function pngForGlyph(glyph: SysGlyph) {
    const canvas = document.createElement('canvas'); canvas.width = glyph.width; canvas.height = glyph.height;
    const context = canvas.getContext('2d')!; const image = context.createImageData(glyph.width, glyph.height);
    for (let index = 0; index < glyph.pixels.length; index += 1) {
      const pixel = index * 4; const visible = glyph.pixels[index] === 0;
      image.data[pixel] = 0; image.data[pixel + 1] = 0; image.data[pixel + 2] = 0; image.data[pixel + 3] = visible ? 255 : 0;
    }
    context.putImageData(image, 0, 0);
    return new Promise<Blob>((resolve, reject) => canvas.toBlob((blob) => blob ? resolve(blob) : reject(new Error('Could not encode PNG.')), 'image/png'));
  }

  function crc32(data: Uint8Array) {
    let crc = 0xffffffff;
    for (const byte of data) { crc ^= byte; for (let bit = 0; bit < 8; bit += 1) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0); }
    return (crc ^ 0xffffffff) >>> 0;
  }
  const u16 = (value: number) => Uint8Array.of(value & 255, (value >>> 8) & 255);
  const u32 = (value: number) => Uint8Array.of(value & 255, (value >>> 8) & 255, (value >>> 16) & 255, (value >>> 24) & 255);
  function join(parts: Uint8Array[]) { const length = parts.reduce((total, part) => total + part.length, 0); const out = new Uint8Array(length); let offset = 0; for (const part of parts) { out.set(part, offset); offset += part.length; } return out; }
  function storedZip(entries: { name: string; bytes: Uint8Array }[]) {
    const encoder = new TextEncoder(); const locals: Uint8Array[] = []; const central: Uint8Array[] = []; let offset = 0;
    for (const entry of entries) {
      const name = encoder.encode(entry.name); const crc = crc32(entry.bytes);
      const local = join([u32(0x04034b50), u16(20), u16(0), u16(0), u16(0), u16(0), u32(crc), u32(entry.bytes.length), u32(entry.bytes.length), u16(name.length), u16(0), name, entry.bytes]);
      locals.push(local);
      central.push(join([u32(0x02014b50), u16(20), u16(20), u16(0), u16(0), u16(0), u16(0), u32(crc), u32(entry.bytes.length), u32(entry.bytes.length), u16(name.length), u16(0), u16(0), u16(0), u16(0), u32(0), u32(offset), name]));
      offset += local.length;
    }
    const directory = join(central); const ending = join([u32(0x06054b50), u16(0), u16(0), u16(entries.length), u16(entries.length), u32(directory.length), u32(offset), u16(0)]);
    return new Blob([join([...locals, directory, ending])], { type: 'application/zip' });
  }

  async function exportVariant() {
    if (!font) return;
    status = `Encoding variant ${variant} PNGs…`;
    try {
      const entries = await Promise.all(font.glyphs.slice(variant * 128, variant * 128 + 128).map(async (glyph, index) => ({ name: `${variant * 128 + index}.png`, bytes: new Uint8Array(await (await pngForGlyph(glyph)).arrayBuffer()) })));
      download(storedZip(entries), `sysfont-variant-${variant}.zip`);
      status = `Exported variant ${variant}: 128 numbered transparent PNGs.`;
    } catch (cause) { error = cause instanceof Error ? cause.message : String(cause); }
  }

  async function importImages(files: FileList | File[]) {
    if (!font) return;
    error = ''; let applied = 0; const rejected: string[] = [];
    for (const file of Array.from(files)) {
      const match = /^(\d+)\.png$/i.exec(file.name);
      const index = match ? Number(match[1]) : NaN;
      if (!Number.isInteger(index) || index < 0 || index >= font.count) { rejected.push(file.name); continue; }
      try {
        const bitmap = await createImageBitmap(file);
        if (!bitmap.width || !bitmap.height || bitmap.width > 96 || bitmap.height > 96) throw new Error('dimensions must be 1–96px');
        const canvas = document.createElement('canvas'); canvas.width = bitmap.width; canvas.height = bitmap.height;
        const context = canvas.getContext('2d', { willReadFrequently: true })!; context.drawImage(bitmap, 0, 0);
        const rgba = context.getImageData(0, 0, bitmap.width, bitmap.height).data;
        const pixels = new Uint8Array(bitmap.width * bitmap.height);
        for (let pixel = 0; pixel < pixels.length; pixel += 1) pixels[pixel] = rgba[pixel * 4 + 3] ? 0 : 255;
        bitmap.close();
        const glyph = { width: canvas.width, height: canvas.height, pixels };
        const glyphs = [...font.glyphs]; glyphs[index] = glyph; font = { ...font, glyphs };
        modified = new Set([...modified, index]); applied += 1;
      } catch (cause) { rejected.push(`${file.name} (${cause instanceof Error ? cause.message : String(cause)})`); }
    }
    status = `Applied ${applied} PNG glyph replacement${applied === 1 ? '' : 's'}${rejected.length ? `; skipped ${rejected.length}` : ''}.`;
    if (rejected.length) error = `Skipped: ${rejected.join(', ')}`;
  }

  function drop(event: DragEvent) { event.preventDefault(); dragging = false; if (event.dataTransfer?.files.length) void importImages(event.dataTransfer.files); }
  function exportFont() {
    if (!font) return;
    try { download(new Blob([rebuildSysFont(font)], { type: 'application/octet-stream' }), 'sysfont-modified.dat'); status = `Exported and verified sysfont-modified.dat (${modified.size} changed glyphs).`; }
    catch (cause) { error = cause instanceof Error ? cause.message : String(cause); }
  }
</script>

<svelte:window ondragover={(event) => event.preventDefault()} ondrop={drop} />
<main class="font-page">
  <header class="font-header"><div><p class="eyebrow">DORAEMON MONOPOLY</p><h1>Sysfont inspector</h1><p class="subtle">Inspect and replace the five 128-slot, single-byte font variants.</p></div><div class="header-actions"><a class="load-button" href="/" data-route>String translator</a><a class="load-button" href="/assets" data-route>Asset viewer</a></div></header>
  <p class="status">{status}</p>{#if error}<p class="error">{error}</p>{/if}
  {#if font}
    <nav class="font-tabs" aria-label="Font variant">{#each Array(font.variants) as _, index}<button class:active={variant === index} onclick={() => variant = index}>Variant {index} <small>{index * 128}–{index * 128 + 127}</small></button>{/each}</nav>
    <section class:dragging class="font-import" role="group" aria-label="Sysfont PNG import" ondragover={(event) => { event.preventDefault(); dragging = true; }} ondragleave={() => dragging = false} ondrop={drop}><strong>Drop replacement PNGs here</strong><span>Name each image by its absolute glyph index: <code>0.png</code> through <code>639.png</code>. Transparent pixels become background; every non-transparent pixel becomes a drawn black font pixel.</span></section>
    <div class="font-actions"><button type="button" onclick={exportVariant}>Export variant {variant} PNGs</button><button type="button" class="primary" disabled={!modified.size} onclick={exportFont}>Export modified sysfont.dat</button><span>{modified.size} modified glyphs</span></div>
    <p class="subtle font-note">Each slot is <code>variant × 128 + byte</code>. Width and height come from each image; exported PNGs use transparent backgrounds.</p>
    <section class="font-grid">{#each font.glyphs.slice(variant * 128, variant * 128 + 128) as glyph, index}<article class:modified={modified.has(variant * 128 + index)}><div class="glyph-preview"><SysfontGlyph {glyph} /></div><code>#{(variant * 128 + index).toString().padStart(3, '0')}</code><strong>{label(index)}</strong><small>{glyph.width} × {glyph.height}px</small></article>{/each}</section>
  {/if}
</main>
