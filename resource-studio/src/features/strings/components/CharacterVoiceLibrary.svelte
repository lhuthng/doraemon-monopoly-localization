<script lang="ts">
  import type { PreparedVoiceRecord } from '../voice';
  import VoiceOnlyRecord from './VoiceOnlyRecord.svelte';

  let {
    voices,
    originalById,
    replacementUrls,
    replacementDurations,
    replacements,
    isModified,
    detailsFor,
    onReplace,
    onReset,
    onLoadOriginal,
    onLoadWorking
  }: {
    voices: PreparedVoiceRecord[];
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
    onReplace: (id: string, file: File) => void;
    onReset: (id: string) => void;
    onLoadOriginal: (id: string) => void;
    onLoadWorking: (id: string) => void;
  } = $props();
</script>

<section class="voice-only-section" aria-label="Character voice library">
  <div class="voice-only-heading">
    <h2>Voice library</h2>
    <span>{voices.length} slots</span>
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
        onReplace={(file) => onReplace(voice.id, file)}
        onReset={() => onReset(voice.id)}
        onLoadOriginal={() => onLoadOriginal(voice.id)}
        onLoadWorking={() => onLoadWorking(voice.id)}
      />
    {/each}
  </div>
</section>
