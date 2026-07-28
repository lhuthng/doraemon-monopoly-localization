<script>
  let { open = false, onClose, onRecorded } = $props();
  let recording = $state(false);
  let error = $state('');
  let elapsed = $state(0);
  let recorder;
  let chunks = [];
  let timer;

  async function start() {
    error = '';
    if (!navigator.mediaDevices?.getUserMedia || !window.MediaRecorder) {
      error = 'This browser does not support microphone recording.';
      return;
    }
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const mimeType = ['audio/webm;codecs=opus', 'audio/ogg;codecs=opus', 'audio/webm'].find((type) =>
        MediaRecorder.isTypeSupported(type)
      );
      recorder = new MediaRecorder(stream, mimeType ? { mimeType } : undefined);
      chunks = [];
      elapsed = 0;
      recorder.ondataavailable = (event) => event.data.size && chunks.push(event.data);
      recorder.onstop = () => {
        stream.getTracks().forEach((track) => track.stop());
        clearInterval(timer);
        const blob = new Blob(chunks, { type: recorder.mimeType || 'audio/webm' });
        recording = false;
        onClose();
        onRecorded(blob);
      };
      recorder.start();
      recording = true;
      timer = window.setInterval(() => (elapsed += 1), 1000);
    } catch (cause) {
      error =
        cause instanceof Error && cause.name === 'NotAllowedError'
          ? 'Microphone permission was denied.'
          : 'Could not start microphone recording.';
    }
  }

  function stop() {
    if (recorder?.state === 'recording') recorder.stop();
  }
</script>

{#if open}
  <div
    class="audio-modal-backdrop"
    role="presentation"
    onclick={(event) => event.target === event.currentTarget && !recording && onClose()}
  >
    <div class="audio-modal" role="dialog" aria-modal="true" aria-labelledby="recorder-title">
      <div class="flex items-start justify-between gap-4">
        <div>
          <p class="text-xs font-black uppercase tracking-[0.18em] text-navy">Voice recorder</p>
          <h2 id="recorder-title" class="mt-1 text-xl font-black text-ink">Record a replacement</h2>
        </div>
        <button
          class="icon-action"
          disabled={recording}
          aria-label="Close recorder"
          title="Close"
          onclick={onClose}>×</button
        >
      </div>
      <p class="mt-4 text-sm leading-6 text-ink/70">
        Your microphone audio stays in this browser. Stop when the take is complete, then trim it before using
        it.
      </p>
      <div class="mt-6 flex items-center justify-center rounded-2xl bg-sky/20 p-8">
        <span
          class:recording-pulse={recording}
          class="grid h-20 w-20 place-items-center rounded-full bg-accent-yellow text-4xl text-navy">●</span
        >
      </div>
      <p class="mt-4 text-center font-mono text-lg font-black text-navy">
        {String(Math.floor(elapsed / 60)).padStart(2, '0')}:{String(elapsed % 60).padStart(2, '0')}
      </p>
      {#if error}<p class="mt-3 rounded-xl bg-danger/10 p-3 text-sm font-bold text-danger">{error}</p>{/if}
      <div class="mt-5 flex justify-end gap-2">
        {#if recording}<button class="action-button bg-danger text-white" onclick={stop}
            >■ Stop recording</button
          >{:else}<button class="action-button bg-accent-blue text-white" onclick={start}
            >● Start recording</button
          >{/if}
      </div>
    </div>
  </div>
{/if}
