<script lang="ts">
  import { onMount } from 'svelte';
  import SysfontGlyph from './SysfontGlyph.svelte';
  import { parseSysFont, type SysFont } from './lib/formats';
  let font: SysFont | undefined = $state();
  let variant = $state(0);
  let error = $state('');
  let status = $state('Loading bundled sysfont.dat…');
  onMount(async () => {
    try {
      const response = await fetch('/sysfont.dat');
      if (!response.ok) throw new Error(`sysfont.dat returned HTTP ${response.status}.`);
      font = parseSysFont(new Uint8Array(await response.arrayBuffer()));
      status = `${font.count} glyphs · ${font.variants} variants · 128 slots per variant`;
    } catch (cause) { error = cause instanceof Error ? cause.message : String(cause); status = 'Loading failed.'; }
  });
  function label(code: number) {
    if (code === 0) return 'NUL';
    if (code === 9) return 'TAB';
    if (code === 10) return 'LF';
    if (code === 13) return 'CR';
    if (code === 32) return 'SPACE';
    return code >= 33 && code < 127 ? JSON.stringify(String.fromCharCode(code)) : `0x${code.toString(16).padStart(2, '0')}`;
  }
</script>

<main class="font-page">
  <header class="font-header"><div><p class="eyebrow">DORAEMON MONOPOLY</p><h1>Sysfont inspector</h1><p class="subtle">Inspect the five 128-slot single-byte font variants used for proportional text.</p></div><div class="header-actions"><a class="load-button" href="/" data-route>String translator</a><a class="load-button" href="/assets" data-route>Asset viewer</a></div></header>
  <p class="status">{status}</p>
  {#if error}<p class="error">{error}</p>{/if}
  {#if font}
    <nav class="font-tabs" aria-label="Font variant">{#each Array(font.variants) as _, index}<button class:active={variant === index} onclick={() => variant = index}>Variant {index} <small>{index * 128}–{index * 128 + 127}</small></button>{/each}</nav>
    <p class="subtle font-note">Each slot is <code>variant × 128 + byte</code>. Width and height come from the glyph record; the byte value is the character code used by the game.</p>
    <section class="font-grid">{#each font.glyphs.slice(variant * 128, variant * 128 + 128) as glyph, index}<article><div class="glyph-preview"><SysfontGlyph {glyph} /></div><code>#{(variant * 128 + index).toString().padStart(3, '0')}</code><strong>{label(index)}</strong><small>{glyph.width} × {glyph.height}px</small></article>{/each}</section>
  {/if}
</main>
