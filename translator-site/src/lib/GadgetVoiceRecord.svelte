<script>
  import AudioDropOverlay from '$lib/AudioDropOverlay.svelte';
  import AudioInputFlow from '$lib/AudioInputFlow.svelte';
  let {
    record,
    voiceId,
    ownerLabel,
    sourceLine,
    translationLine,
    gadgetSrc,
    hasVoice,
    edited = false,
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

<article
  class:voice-edited={edited}
  class:audio-drop-target={dragging}
  class:audio-drop-complete={dropped}
  class="record-compact game-panel relative overflow-hidden p-4"
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
  <div class="grid grid-cols-[4.5rem_minmax(0,1fr)] gap-3">
    <div class="flex w-[4.5rem] shrink-0 flex-col items-center gap-1">
      <div class="grid h-16 w-16 place-items-center rounded-2xl border-2 border-navy bg-white shadow-sm">
        <img class="h-13 w-13 object-contain" src={gadgetSrc} alt="" />
      </div>
      <span class="rounded-full bg-sky/35 px-2 py-1 text-[0.65rem] font-black text-navy">Gadget</span>
    </div>
    <div class="min-w-0 flex-1">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <p class="font-mono text-sm font-black text-accent-blue">{voiceId}</p>
        <div class="absolute right-4 top-4 flex gap-2">
          {#if edited}<span class="rounded-full bg-accent-yellow px-2 py-1 text-xs font-black text-navy"
              >Audio edited</span
            >{/if}
        </div>
      </div>
      <h2 class="mt-2 text-lg font-black text-ink">{ownerLabel} · Gadget voice {record.gadgetVoiceSlot}</h2>
      <p class="mt-2 text-sm text-ink/80"><span class="font-bold">Original:</span> {sourceLine}</p>
      <p class="mt-1 text-sm text-ink/80">
        <span class="font-bold">Translation:</span>
        {translationLine || '—'}
      </p>
      <div class="mt-4 flex flex-wrap items-center gap-2">
        <button
          class="action-button border-outline bg-ice-panel text-navy disabled:cursor-not-allowed disabled:opacity-40"
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
  </div>
</article>
