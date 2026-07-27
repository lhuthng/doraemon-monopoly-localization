<script>
  import AudioDropOverlay from '$lib/AudioDropOverlay.svelte';
  import AudioInputFlow from '$lib/AudioInputFlow.svelte';
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
  let dropped = $state(false);
  let audioFlow = $state();

  function dropAudio(event) {
    event.preventDefault();
    event.stopPropagation();
    dragging = false;
    dropped = true;
    window.setTimeout(() => (dropped = false), 650);
    const file = event.dataTransfer?.files?.[0];
    event.dataTransfer?.clearData();
    if (file?.type.startsWith('audio/') || /\.(wav|mp3|flac|ogg|opus|m4a|aac)$/i.test(file?.name ?? '')) {
      audioFlow?.openFile(file);
    }
  }
</script>

<div
  class:voice-edited={edited}
  class:audio-drop-target={dragging}
  class:audio-drop-complete={dropped}
  class="record-compact rounded-2xl border border-outline/70 bg-white p-3"
  role="region"
  aria-label={title}
  ondragover={(event) => {
    event.preventDefault();
    event.stopPropagation();
    dragging = true;
  }}
  ondragleave={(event) => {
    event.stopPropagation();
    if (!event.relatedTarget || !event.currentTarget.contains(event.relatedTarget)) dragging = false;
  }}
  ondrop={dropAudio}
>
  <div class="flex items-center justify-between gap-2">
    <p class="text-sm font-black text-navy">{title}</p>
    {#if edited}<span class="rounded-full bg-accent-yellow px-2 py-1 text-xs font-black text-navy"
        >Voice edited</span
      >{/if}
  </div>
  <p class="mt-1 font-mono text-xs text-ink/55">{voiceId} · {detail}</p>
  <div class="mt-3 flex flex-wrap items-center gap-2">
    <button
      class="action-button border-outline bg-ice-panel text-navy disabled:opacity-40"
      disabled={!hasVoice}
      onclick={onPlayOriginal}>▶ 1</button
    ><button
      class="action-button border-outline bg-ice-panel text-navy disabled:opacity-40"
      disabled={!edited}
      onclick={onPlayNew}>▶ 2</button
    ><button
      class="icon-action disabled:opacity-40"
      disabled={!edited}
      title="Remove replacement audio"
      aria-label="Remove replacement audio"
      onclick={onRemove}>⌫</button
    ><AudioInputFlow bind:this={audioFlow} onUseAudio={onReplace} />
    <label
      class="hidden icon-action cursor-pointer bg-accent-blue text-white hover:text-white"
      title="Replace audio"
      aria-label="Replace audio"
      ><svg aria-hidden="true" viewBox="0 0 24 24" class="h-4 w-4"
        ><path
          d="M7 7a7 7 0 0 1 11.5 2M17 17a7 7 0 0 1-11.5-2M18 5v4h-4M6 19v-4h4"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        /></svg
      ><input
        class="hidden"
        type="file"
        accept="audio/*,.wav,.mp3,.flac,.ogg,.opus,.m4a,.aac"
        onchange={(event) => onReplace(event.currentTarget.files?.[0])}
      /></label
    >{#if dragging}<AudioDropOverlay {dragging} />{/if}
  </div>
</div>
