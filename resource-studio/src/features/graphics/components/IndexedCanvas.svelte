<script lang="ts">
  import { diagnosticPalette, type IndexedImage, type Palette } from '../../../lib/asset-formats';

  let {
    image,
    palette,
    scale = 1,
    fitVisible = false
  }: { image: IndexedImage; palette?: Palette; scale?: number; fitVisible?: boolean } = $props();
  let canvas: HTMLCanvasElement;

  function visibleBounds(source: IndexedImage) {
    if (!fitVisible || !source.alpha) return { x: 0, y: 0, width: source.width, height: source.height };
    let left = source.width;
    let top = source.height;
    let right = -1;
    let bottom = -1;
    for (let index = 0; index < source.alpha.length; index += 1) {
      if (!source.alpha[index]) continue;
      const x = index % source.width;
      const y = Math.floor(index / source.width);
      left = Math.min(left, x);
      top = Math.min(top, y);
      right = Math.max(right, x);
      bottom = Math.max(bottom, y);
    }
    return right < left
      ? { x: 0, y: 0, width: source.width, height: source.height }
      : { x: left, y: top, width: right - left + 1, height: bottom - top + 1 };
  }

  const bounds = $derived(visibleBounds(image));

  $effect(() => {
    if (!canvas || !image) return;
    const colors = image.palette ?? palette ?? diagnosticPalette();
    const context = canvas.getContext('2d');
    if (!context) return;
    const output = context.createImageData(bounds.width, bounds.height);
    for (let y = 0; y < bounds.height; y += 1) {
      for (let x = 0; x < bounds.width; x += 1) {
        const source = (bounds.y + y) * image.width + bounds.x + x;
        const color = image.pixels[source] * 3;
        const pixel = (y * bounds.width + x) * 4;
        output.data[pixel] = colors[color];
        output.data[pixel + 1] = colors[color + 1];
        output.data[pixel + 2] = colors[color + 2];
        output.data[pixel + 3] = image.alpha?.[source] ?? 255;
      }
    }
    context.putImageData(output, 0, 0);
  });
</script>

<canvas
  bind:this={canvas}
  width={bounds.width}
  height={bounds.height}
  style:width={`${bounds.width * scale}px`}
  style:height={`${bounds.height * scale}px`}
></canvas>
