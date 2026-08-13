<script lang="ts">
  import type { ChiGlyph } from '@doraemon-monopoly/dubbing-core';
  let { glyph, scale = 3 }: { glyph: ChiGlyph; scale?: number } = $props();
  let canvas: HTMLCanvasElement;
  $effect(() => {
    const context = canvas?.getContext('2d');
    if (!context) return;
    canvas.width = 16;
    canvas.height = 16;
    const image = context.createImageData(16, 16);
    for (let index = 0; index < glyph.pixels.length; index += 1) {
      const pixel = index * 4;
      const ink = glyph.pixels[index];
      image.data[pixel] = 0;
      image.data[pixel + 1] = 0;
      image.data[pixel + 2] = 0;
      image.data[pixel + 3] = ink ? 255 : 0;
    }
    context.putImageData(image, 0, 0);
  });
</script>

<canvas
  bind:this={canvas}
  width="16"
  height="16"
  style:width={`${16 * scale}px`}
  style:height={`${16 * scale}px`}
></canvas>
