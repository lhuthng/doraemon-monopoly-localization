<script>
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

  function dropAudio(event) {
    event.preventDefault();
    dragging = false;
    const file = event.dataTransfer?.files?.[0];
    if (file) onReplace(file);
  }
</script>

<article
  class:voice-edited={edited}
  class:audio-drop-target={dragging}
  class="game-panel overflow-hidden p-4 sm:p-5"
  ondragover={(event) => {
    event.preventDefault();
    dragging = true;
  }}
  ondragleave={() => (dragging = false)}
  ondrop={dropAudio}
>
  <div class="flex gap-4">
    <div
      class="grid h-16 w-16 shrink-0 place-items-center rounded-2xl border-2 border-navy bg-white shadow-sm"
    >
      <img class="h-13 w-13 object-contain" src={gadgetSrc} alt="" />
    </div>
    <div class="min-w-0 flex-1">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <p class="font-mono text-sm font-black text-accent-blue">{voiceId}</p>
        <div class="flex gap-2">
          <span class="rounded-full bg-sky/35 px-2 py-1 text-xs font-black text-navy">Gadget</span
          >{#if edited}<span class="rounded-full bg-accent-yellow px-2 py-1 text-xs font-black text-navy"
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
      <div class="mt-4 flex flex-wrap gap-2">
        <button
          class="action-button border-outline bg-ice-panel text-navy disabled:cursor-not-allowed disabled:opacity-40"
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
  </div>
</article>
