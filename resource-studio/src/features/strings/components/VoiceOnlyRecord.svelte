<script lang="ts">
  import type { PreparedVoiceRecord } from '../voice';
  import VoiceEditor from './VoiceEditor.svelte';

  let {
    voice,
    original,
    category,
    detail,
    symbol,
    originalName,
    translatedName,
    replacementUrl,
    replacementDuration,
    cleared = false,
    modified = false,
    onReplace,
    onReset,
    onLoadOriginal,
    onLoadWorking
  }: {
    voice: PreparedVoiceRecord;
    original?: PreparedVoiceRecord;
    category: string;
    detail: string;
    symbol?: string;
    originalName?: string;
    translatedName?: string;
    replacementUrl?: string;
    replacementDuration?: number;
    cleared?: boolean;
    modified?: boolean;
    onReplace: (file: File) => void;
    onReset: () => void;
    onLoadOriginal: () => void;
    onLoadWorking: () => void;
  } = $props();
</script>

<article class:done={modified} class="voice-record-card">
  <div class="record-heading">
    <code>{voice.id}</code>
    <span class="voice-category">{category}</span>
  </div>
  <div class="voice-record-identity">
    {#if symbol}<strong class="voice-symbol">{symbol}</strong>{/if}
    <div>
      <strong>{detail}</strong>
      {#if originalName}<span lang="zh-Hant">Original: {originalName}</span>{/if}
      {#if translatedName}<span>Translation: {translatedName}</span>{/if}
    </div>
  </div>
  <VoiceEditor
    compact
    {original}
    working={voice}
    {replacementUrl}
    {replacementDuration}
    {cleared}
    {modified}
    {onReplace}
    {onReset}
    {onLoadOriginal}
    {onLoadWorking}
  />
</article>
