<!-- eslint-disable svelte/no-navigation-without-resolve -->
<script>
  import AudioDropOverlay from '$lib/AudioDropOverlay.svelte';
  import AudioInputFlow from '$lib/AudioInputFlow.svelte';
  let {
    record,
    source,
    translation,
    language,
    edited = false,
    voiceEdited = false,
    voiceId,
    hasVoice,
    translateHref,
    onTranslation,
    onReflow,
    onPlayOriginal,
    onPlayNew,
    onReplace,
    onRemove,
    onReset
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
  class:record-edited={edited || voiceEdited}
  class:audio-drop-target={dragging}
  class:audio-drop-complete={dropped}
  class="record-compact game-panel relative overflow-hidden"
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
  <div
    class="flex flex-wrap items-center justify-between gap-2 border-b border-outline bg-ice-panel px-5 py-3"
  >
    <strong class="font-mono text-sm text-navy">{record.id}</strong>
    <div class="absolute right-4 top-3 flex gap-2">
      {#if edited}<span class="rounded-full bg-success/15 px-3 py-1 text-xs font-black text-success"
          >Dialogue edited</span
        >{/if}
      {#if voiceEdited}<span class="rounded-full bg-accent-yellow px-3 py-1 text-xs font-black text-navy"
          >Audio edited</span
        >{/if}
      {#if dragging}<AudioDropOverlay {dragging} />{/if}
    </div>
  </div>
  <div class="p-5">
    <div class="space-y-3">
      <div class="flex min-h-36 flex-col rounded-2xl border border-outline/70 bg-sky/15 p-4">
        <p class="text-xs font-black uppercase tracking-[0.16em] text-navy">Original game text</p>
        <p class="mt-2 whitespace-pre-wrap text-sm leading-6 text-ink">{source}</p>
        <button
          class="mt-auto self-end pt-3 text-xs! font-black text-accent-blue hover:text-navy hover:underline"
          onclick={() => window.open(translateHref, '_blank', 'noopener,noreferrer')}>GGTranslate ↗</button
        >
      </div>
      <label class="block rounded-2xl border border-success/35 bg-emerald-50/55 p-4">
        <span class="text-xs font-black uppercase tracking-[0.16em] text-success">{language} translation</span
        >
        <textarea
          class="mt-2 min-h-28 w-full resize-y border-0 bg-transparent p-0 text-sm leading-6 text-ink outline-none placeholder:text-ink/35 focus:ring-0"
          aria-label={`Translation for ${record.id}`}
          value={translation}
          oninput={(event) => onTranslation(event.currentTarget.value)}
          placeholder="Write the translation"
        ></textarea>
      </label>
    </div>
    <div class="mt-3 flex flex-wrap items-center justify-start gap-2">
      <button
        class="icon-action"
        title="Reflow text"
        aria-label="Reflow text"
        disabled={!translation.trim()}
        onclick={onReflow}>↔</button
      ><button
        class="icon-action"
        title="Reset dialogue"
        aria-label="Reset dialogue"
        disabled={!edited}
        onclick={onReset}>↶</button
      >
      {#if voiceId}<button
          class="action-button border-outline bg-white text-navy disabled:opacity-40"
          disabled={!hasVoice}
          onclick={onPlayOriginal}>▶ 1</button
        ><button
          class="action-button border-outline bg-white text-navy disabled:opacity-40"
          disabled={!voiceEdited}
          onclick={onPlayNew}>▶ 2</button
        ><button
          class="icon-action disabled:opacity-40"
          disabled={!voiceEdited}
          title="Remove replacement audio"
          aria-label="Remove replacement audio"
          onclick={onRemove}>⌫</button
        >{/if}
      {#if voiceId}
        <AudioInputFlow bind:this={audioFlow} onUseAudio={onReplace} />
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
        >
      {/if}
    </div>
  </div>
</article>
