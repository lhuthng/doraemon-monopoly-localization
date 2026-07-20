<script lang="ts">
  import type { PreparedVoiceRecord } from '../voice';

  let {
    original,
    working,
    replacementUrl,
    replacementDuration,
    cleared = false,
    modified = false,
    compact = false,
    onReplace,
    onReset,
    onLoadOriginal,
    onLoadWorking
  }: {
    original?: PreparedVoiceRecord;
    working?: PreparedVoiceRecord;
    replacementUrl?: string;
    replacementDuration?: number;
    cleared?: boolean;
    modified?: boolean;
    compact?: boolean;
    onReplace: (file: File) => void;
    onReset: () => void;
    onLoadOriginal: () => void;
    onLoadWorking: () => void;
  } = $props();

  let dragging = $state(false);
  let current = $derived(cleared ? undefined : replacementUrl || working?.url);
  let voiceState = $derived(
    replacementUrl || modified
      ? 'Modified voice'
      : original?.storage === 'empty'
        ? 'Mapped voice slot is empty'
        : original
          ? 'Original voice'
          : 'Empty voice slot'
  );

  function choose(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    if (input.files?.[0]) onReplace(input.files[0]);
    input.value = '';
  }

  function drop(event: DragEvent) {
    event.preventDefault();
    dragging = false;
    const file = event.dataTransfer?.files[0];
    if (file) onReplace(file);
  }
</script>

<section
  role="group"
  class:compact
  class:dragging
  class="voice-editor"
  ondragenter={() => (dragging = true)}
  ondragleave={() => (dragging = false)}
  ondragover={(event) => event.preventDefault()}
  ondrop={drop}
>
  <div class="voice-editor-heading">
    <span class:modified>{voiceState}</span>
    {#if replacementUrl && replacementDuration !== undefined}
      <small>{replacementDuration.toFixed(2)}s · 22,050 Hz · 16-bit PCM WAV</small>
    {:else if working?.duration}<small
        >{working.duration.toFixed(2)}s · {working.sampleRate?.toLocaleString()} Hz · {working.bitsPerSample}-bit
        WAV</small
      >{/if}
  </div>
  <div class="voice-playback-grid">
    <div>
      <small>Original</small>
      {#if original?.url}
        <audio controls preload="none" src={original.url}></audio>
      {:else if original?.storage !== 'empty' && original}
        <button type="button" class="copy" onclick={onLoadOriginal}>Load playback</button>
      {:else}
        <p>No original audio.</p>
      {/if}
    </div>
    {#if modified || replacementUrl || cleared}
      <div>
        <small>{replacementUrl ? 'Replacement' : 'Working copy'}</small>
        {#if current}
          <audio controls preload="none" src={current}></audio>
        {:else if !cleared && working?.storage !== 'empty' && working}
          <button type="button" class="copy" onclick={onLoadWorking}>Load playback</button>
        {:else}
          <p>No audio is stored in this slot.</p>
        {/if}
      </div>
    {/if}
  </div>
  <div class="voice-editor-actions">
    <label class="small-button"
      >Replace audio<input
        type="file"
        accept="audio/*,.wav,.mp3,.flac,.ogg,.opus,.m4a,.aac"
        onchange={choose}
      /></label
    >
    {#if modified || replacementUrl}<button type="button" class="copy" onclick={onReset}
        >Restore original</button
      >{/if}
  </div>
</section>
