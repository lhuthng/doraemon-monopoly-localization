<script>
  let {
    title,
    detail,
    voiceId,
    edited = false,
    hasVoice = false,
    onPlayOriginal,
    onPlayNew,
    onReplace,
    onRemove
  } = $props();
  let dragging = $state(false);

  function dropAudio(event) {
    event.preventDefault();
    dragging = false;
    const file = event.dataTransfer?.files?.[0];
    if (file) onReplace(file);
  }
</script>

<div
  class:voice-edited={edited}
  class:audio-drop-target={dragging}
  class="rounded-2xl border border-outline/70 bg-white p-4"
  role="region"
  aria-label={title}
  ondragover={(event) => {
    event.preventDefault();
    dragging = true;
  }}
  ondragleave={() => (dragging = false)}
  ondrop={dropAudio}
>
  <div class="flex items-center justify-between gap-2">
    <p class="text-sm font-black text-navy">{title}</p>
    {#if edited}<span class="rounded-full bg-accent-yellow px-2 py-1 text-xs font-black text-navy"
        >Voice edited</span
      >{/if}
  </div>
  <p class="mt-1 font-mono text-xs text-ink/55">{voiceId} · {detail}</p>
  <div class="mt-3 flex flex-wrap gap-2">
    <button
      class="action-button border-outline bg-ice-panel text-navy disabled:opacity-40"
      disabled={!hasVoice}
      onclick={onPlayOriginal}>▶ Original</button
    ><button
      class="action-button border-outline bg-ice-panel text-navy disabled:opacity-40"
      disabled={!edited}
      onclick={onPlayNew}>▶ New</button
    ><button
      class="icon-action disabled:opacity-40"
      disabled={!edited}
      title="Remove replacement audio"
      aria-label="Remove replacement audio"
      onclick={onRemove}>⌫</button
    ><label class="action-button cursor-pointer bg-accent-blue text-white hover:text-white"
      >Replace<input
        class="hidden"
        type="file"
        accept="audio/*,.wav,.mp3,.flac,.ogg,.opus,.m4a,.aac"
        onchange={(event) => onReplace(event.currentTarget.files?.[0])}
      /></label
    >{#if dragging}<span class="rounded-xl bg-accent-blue px-3 py-2 text-xs font-black text-white"
        >Drop audio to replace</span
      >{/if}
  </div>
</div>
