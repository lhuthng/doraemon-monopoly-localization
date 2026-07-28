<script>
  import AudioEditorPopover from '$lib/AudioEditorPopover.svelte';
  import VoiceRecorderPopover from '$lib/VoiceRecorderPopover.svelte';
  let { onUseAudio } = $props();
  let recorderOpen = $state(false);
  let editorOpen = $state(false);
  let source = $state();
  export function openFile(file) {
    if (file) {
      source = file;
      editorOpen = true;
    }
  }
  function recorded(blob) {
    source = blob;
    editorOpen = true;
  }
</script>

<button
  class="icon-action cursor-pointer bg-accent-blue text-white"
  title="Record audio"
  aria-label="Record audio"
  onclick={() => (recorderOpen = true)}>⌁</button
>
<VoiceRecorderPopover open={recorderOpen} onClose={() => (recorderOpen = false)} onRecorded={recorded} />
<AudioEditorPopover open={editorOpen} {source} onClose={() => (editorOpen = false)} onUse={onUseAudio} />
