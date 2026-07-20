<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { StringRecord } from '../../../lib/formats';
  import { DIALOG_LAYOUT, GADGETS_LAYOUT } from '../text-layout';

  let {
    record,
    source,
    translation,
    generationState,
    archived = false,
    queued = false,
    translating = false,
    origin = '',
    locked = false,
    copied = false,
    onRegenerate,
    onReflow,
    onFlatten,
    onCopy,
    onTranslation,
    children
  }: {
    record: StringRecord;
    source: string;
    translation: string;
    generationState?: string;
    archived?: boolean;
    queued?: boolean;
    translating?: boolean;
    origin?: string;
    locked?: boolean;
    copied?: boolean;
    onRegenerate: () => void;
    onReflow: (width: number, preset: 'gadgets' | 'dialog') => void;
    onFlatten: () => void;
    onCopy: () => void;
    onTranslation: (value: string) => void;
    children?: Snippet;
  } = $props();

  let width = $state<number>(GADGETS_LAYOUT.maxWidth);
  let preset = $state<'gadgets' | 'dialog'>('gadgets');
  let popoverId = $derived(`reflow-${record.id.replace('/', '-')}`);
</script>

<article
  id={`record-${record.id.replace('/', '-')}`}
  class:queued
  class:translating
  class:done={!!translation.trim()}
  class:manual={origin === 'manual'}
  class:generated={origin === 'generated'}
  class:imported={origin === 'imported'}
>
  <div class="record-heading">
    <code>{record.id}</code>
    <div class="record-actions">
      {#if generationState}<span class="record-state">{generationState}</span>{/if}
      {#if archived}<span class="record-state archived-voice-state"
          >Archived voice · not played by stock game</span
        >{/if}
      {#if translation.trim()}
        <button
          type="button"
          class="icon-button"
          aria-label="Regenerate translation"
          title="Regenerate translation"
          onclick={onRegenerate}>↻</button
        >
      {/if}
      <button
        type="button"
        class="icon-button"
        aria-label="Reflow text"
        title="Reflow text"
        disabled={!translation.trim() || locked}
        popovertarget={popoverId}>↔</button
      >
      <button
        type="button"
        class="icon-button"
        aria-label="Flatten lines"
        title="Flatten lines"
        disabled={!translation.trim() || locked}
        onclick={onFlatten}>≡</button
      >
      <button type="button" class="icon-button" aria-label="Copy source" title="Copy source" onclick={onCopy}
        >{copied ? '✓' : '⧉'}</button
      >
    </div>
  </div>
  <div id={popoverId} class="reflow-popover" popover>
    <strong>Reflow {record.id}</strong>
    <label>Maximum width (px)<input min="1" max="999" type="number" bind:value={width} /></label>
    <div class="reflow-popover-actions">
      <button
        type="button"
        class="quiet"
        onclick={() => {
          preset = 'gadgets';
          width = GADGETS_LAYOUT.maxWidth;
        }}>Gadgets preset · 87px</button
      >
      <button
        type="button"
        class="quiet"
        onclick={() => {
          preset = 'dialog';
          width = DIALOG_LAYOUT.maxWidth;
        }}>Dialog preset · 264px</button
      >
      <button type="button" class="primary" onclick={() => onReflow(width, preset)}>Reflow text</button>
    </div>
  </div>
  <pre class="source-text" lang="zh-Hant">{source}</pre>
  <label>
    Translation
    <textarea
      rows={Math.max(2, source.split('\n').length)}
      disabled={locked}
      placeholder="Enter translation…"
      value={translation}
      oninput={(event) => onTranslation(event.currentTarget.value)}></textarea>
  </label>
  {#if children}{@render children()}{/if}
</article>
