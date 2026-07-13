<script lang="ts">
  import { diagnosticPalette, type IndexedImage, type Palette } from './lib/asset-formats';

  let { image, palette, scale = 1 }: { image: IndexedImage; palette?: Palette; scale?: number } = $props();
  let canvas: HTMLCanvasElement;

  $effect(() => {
    if (!canvas || !image) return;
    const colors = image.palette ?? palette ?? diagnosticPalette();
    const context = canvas.getContext('2d');
    if (!context) return;
    const output = context.createImageData(image.width, image.height);
    for (let index = 0; index < image.pixels.length; index += 1) {
      const color = image.pixels[index] * 3;
      const pixel = index * 4;
      output.data[pixel] = colors[color];
      output.data[pixel + 1] = colors[color + 1];
      output.data[pixel + 2] = colors[color + 2];
      output.data[pixel + 3] = image.alpha?.[index] ?? 255;
    }
    context.putImageData(output, 0, 0);
  });
</script>

<canvas bind:this={canvas} width={image.width} height={image.height} style:width={`${image.width * scale}px`} style:height={`${image.height * scale}px`}></canvas>
