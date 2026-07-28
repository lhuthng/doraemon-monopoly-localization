<script>
  let { dragging = false } = $props();
</script>

{#if dragging}
  <div class="audio-drop-overlay" aria-hidden="true">Drop audio to replace</div>
{/if}
