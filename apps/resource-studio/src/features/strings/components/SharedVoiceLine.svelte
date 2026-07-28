<script lang="ts">
  import type { PreparedVoiceRecord } from '../voice';
  import VoiceOnlyRecord from './VoiceOnlyRecord.svelte';

  let {
    title,
    voices,
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
    title: string;
    voices: PreparedVoiceRecord[];
    characters: string[];
    originalById: Map<string, PreparedVoiceRecord>;
    replacementUrls: Record<string, string>;
    replacementDurations: Record<string, number>;
    replacements: Record<string, Uint8Array | null>;
    isModified: (id: string) => boolean;
    detailsFor: (voice: PreparedVoiceRecord) => {
      category: string;
      detail: string;
      symbol?: string;
      originalName?: string;
      translatedName?: string;
    };
    onJump: (voice: PreparedVoiceRecord) => void;
    onLoadOriginal: (id: string) => void;
    onLoadWorking: (id: string) => void;
  } = $props();
</script>

<section class="voice-line-group" aria-label={title}>
  <div class="voice-line-heading">
    <strong>{title}</strong>
    <span>{voices.length} character recordings</span>
  </div>
  <div class="voice-only-grid">
    {#each voices as voice (voice.id)}
      {@const details = detailsFor(voice)}
      <VoiceOnlyRecord
        {voice}
        original={originalById.get(voice.id)}
        {...details}
        replacementUrl={replacementUrls[voice.id]}
        replacementDuration={replacementDurations[voice.id]}
        cleared={Object.prototype.hasOwnProperty.call(replacements, voice.id) &&
          replacements[voice.id] === null}
        modified={isModified(voice.id)}
        readOnly
        jumpLabel={`Edit ${characters[voice.path[0]] ?? 'character'}`}
        onJump={() => onJump(voice)}
        onReplace={() => undefined}
        onReset={() => undefined}
        onLoadOriginal={() => onLoadOriginal(voice.id)}
        onLoadWorking={() => onLoadWorking(voice.id)}
      />
    {/each}
  </div>
</section>
