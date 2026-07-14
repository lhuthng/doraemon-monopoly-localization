<script lang="ts">
  import type { SysGlyph } from './lib/formats';
  let { glyph, scale = 3 }: { glyph: SysGlyph; scale?: number } = $props();
  let canvas: HTMLCanvasElement;
  $effect(() => {
    const context = canvas?.getContext('2d');
    if (!context) return;
    canvas.width = glyph.width; canvas.height = glyph.height;
    const image = context.createImageData(glyph.width, glyph.height);
    for (let i = 0; i < glyph.pixels.length; i += 1) {
      const p = i * 4;
      // sysfont uses 255 for empty background and 0 for an ink pixel.
      image.data[p] = 0; image.data[p + 1] = 0; image.data[p + 2] = 0; image.data[p + 3] = glyph.pixels[i] === 0 ? 255 : 0;
    }
    context.putImageData(image, 0, 0);
  });
</script>
<canvas bind:this={canvas} style:width={`${glyph.width * scale}px`} style:height={`${glyph.height * scale}px`}></canvas>
