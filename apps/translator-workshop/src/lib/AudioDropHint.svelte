<script>
  let { active = false } = $props();
</script>

{#if active}
  <div class="audio-drop-hint" aria-hidden="true">Drop audio to replace</div>
{/if}
