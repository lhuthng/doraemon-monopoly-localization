<script lang="ts">
  import type { PreparedVoiceRecord } from '../voice';
  import SharedVoiceLine from './SharedVoiceLine.svelte';

  let {
    lines,
    characters,
    originalById,
    replacementUrls,
    replacementDurations,
    replacements,
    isModified,
    detailsFor,
    onJump,
    onLoadOriginal,
    onLoadWorking
  }: {
    lines: PreparedVoiceRecord[][];
    characters: string[];
    originalById: Map<string, PreparedVoiceRecord>;
    replacementUrls: Record<string, string>;
    replacementDurations: Record<string, number>;
    replacements: Record<string, Uint8Array | null>;
    isModified: (id: string) => boolean;
    detailsFor: (voice: PreparedVoiceRecord) => ReturnTypeDetails;
    onJump: (voice: PreparedVoiceRecord) => void;
    onLoadOriginal: (id: string) => void;
    onLoadWorking: (id: string) => void;
  } = $props();

  type ReturnTypeDetails = {
    category: string;
    detail: string;
    symbol?: string;
    originalName?: string;
    translatedName?: string;
  };

  const groups = [
    { label: 'Menu', lines: () => lines.filter((line) => line[0].path[2] <= 10) },
    { label: 'Misc', lines: () => lines.filter((line) => line[0].path[2] >= 16) }
  ];
</script>

<section class="global-voice-library" aria-label="Voices without dialogue text">
  <div class="voice-only-heading">
    <h2>Menu and misc voices</h2>
    <span>Playback here · edit in character groups</span>
  </div>
  {#each groups as group (group.label)}
    {@const groupLines = group.lines()}
    {#if groupLines.length}
      <section class="global-voice-category">
        <h3>{group.label}</h3>
        <div class="voice-line-list">
          {#each groupLines as line (line[0].id)}
            <SharedVoiceLine
              title={`00*/001/${String(line[0].path[2]).padStart(3, '0')}`}
              voices={line}
              {characters}
              {originalById}
              {replacementUrls}
              {replacementDurations}
              {replacements}
              {isModified}
              {detailsFor}
              {onJump}
              {onLoadOriginal}
              {onLoadWorking}
            />
          {/each}
        </div>
      </section>
    {/if}
  {/each}
</section>
