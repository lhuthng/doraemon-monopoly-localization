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
      const value = glyph.pixels[i]; const p = i * 4;
      image.data[p] = value; image.data[p + 1] = value; image.data[p + 2] = value; image.data[p + 3] = 255;
    }
    context.putImageData(image, 0, 0);
  });
</script>
<canvas bind:this={canvas} style:width={`${glyph.width * scale}px`} style:height={`${glyph.height * scale}px`}></canvas>
