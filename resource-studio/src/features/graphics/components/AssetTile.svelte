<script lang="ts">
  import type { IndexedImage, Palette } from '../../../lib/asset-formats';
  import IndexedCanvas from './IndexedCanvas.svelte';

  let {
    image,
    palette,
    fitVisible = false,
    scale = 1,
    modified = false,
    checked = false,
    onopen,
    oncheck
  }: {
    image: IndexedImage;
    palette?: Palette;
    fitVisible?: boolean;
    scale?: number;
    modified?: boolean;
    checked?: boolean;
    onopen?: () => void;
    oncheck?: (checked: boolean) => void;
  } = $props();
</script>

<button class="asset" class:modified onclick={onopen} title={`Open ${image.id}`}>
  <span class="preview"><IndexedCanvas {image} {palette} {fitVisible} {scale} /></span>
  <span class="meta">
    <b>#{image.id}</b>
    {#if oncheck}
      <input
        type="checkbox"
        {checked}
        onclick={(event) => {
          event.stopPropagation();
          oncheck?.(!checked);
        }}
        aria-label={`Select ${image.id}`}
      />
    {/if}
  </span>
</button>
