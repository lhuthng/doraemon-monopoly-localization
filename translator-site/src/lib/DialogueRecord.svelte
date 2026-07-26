<!-- eslint-disable svelte/no-navigation-without-resolve -->
<script>
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

  function dropAudio(event) {
    event.preventDefault();
    dragging = false;
    const file = event.dataTransfer?.files?.[0];
    if (file) onReplace(file);
  }
</script>

<article
  class:record-edited={edited || voiceEdited}
  class:audio-drop-target={dragging}
  class="game-panel overflow-hidden"
  ondragover={(event) => {
    event.preventDefault();
    dragging = true;
  }}
  ondragleave={() => (dragging = false)}
  ondrop={dropAudio}
>
  <div
    class="flex flex-wrap items-center justify-between gap-2 border-b border-outline bg-ice-panel px-5 py-3"
  >
    <strong class="font-mono text-sm text-navy">{record.id}</strong>
    <div class="flex gap-2">
      {#if edited}<span class="rounded-full bg-success/15 px-3 py-1 text-xs font-black text-success"
          >Dialogue edited</span
        >{/if}
      {#if voiceEdited}<span class="rounded-full bg-accent-yellow px-3 py-1 text-xs font-black text-navy"
          >Audio edited</span
        >{/if}
      {#if dragging}<span class="rounded-full bg-accent-blue px-3 py-1 text-xs font-black text-white"
          >Drop audio to replace</span
        >{/if}
    </div>
  </div>
  <div class="p-5">
    <div class="grid gap-3 lg:grid-cols-2">
      <div class="flex min-h-36 flex-col rounded-2xl border border-outline/70 bg-sky/15 p-4">
        <p class="text-xs font-black uppercase tracking-[0.16em] text-navy">Original game text</p>
        <p class="mt-2 whitespace-pre-wrap text-sm leading-6 text-ink">{source}</p>
        <button
          class="mt-auto self-end pt-3 text-xs font-black text-accent-blue hover:text-navy hover:underline"
          onclick={() => window.open(translateHref, '_blank', 'noopener,noreferrer')}
          >Open in Google Translate ↗</button
        >
      </div>
      <label class="rounded-2xl border border-success/35 bg-emerald-50/55 p-4">
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
    <div class="mt-3 flex flex-wrap justify-end gap-2">
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
          onclick={onPlayOriginal}>▶ Original</button
        ><button
          class="action-button border-outline bg-white text-navy disabled:opacity-40"
          disabled={!voiceEdited}
          onclick={onPlayNew}>▶ New</button
        ><button
          class="icon-action disabled:opacity-40"
          disabled={!voiceEdited}
          title="Remove replacement audio"
          aria-label="Remove replacement audio"
          onclick={onRemove}>⌫</button
        >{/if}
      {#if voiceId}
        <label class="action-button cursor-pointer bg-accent-blue text-white hover:text-white"
          >Replace<input
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
