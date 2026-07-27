<script>
  let { open = false, source, onClose, onUse } = $props();
  let audioBuffer = $state();
  let start = $state(0);
  let end = $state(1);
  let duration = $state(1);
  let canvas = $state();
  let timeline = $state();
  let error = $state('');

  $effect(() => {
    if (!open || !source) return;
    let cancelled = false;
    (async () => {
      try {
        const Context = window.AudioContext ?? window.webkitAudioContext;
        const context = new Context();
        const buffer = await context.decodeAudioData(await source.arrayBuffer());
        await context.close();
        if (cancelled) return;
        audioBuffer = buffer;
        duration = buffer.duration;
        start = 0;
        end = buffer.duration;
        drawWaveform(buffer);
      } catch {
        error = 'The browser could not decode this audio file.';
      }
    })();
    return () => (cancelled = true);
  });

  function drawWaveform(buffer) {
    if (!canvas) return;
    const context = canvas.getContext('2d');
    const width = (canvas.width = canvas.clientWidth * devicePixelRatio);
    const height = (canvas.height = canvas.clientHeight * devicePixelRatio);
    context.clearRect(0, 0, width, height);
    context.fillStyle = '#1478df';
    const data = buffer.getChannelData(0);
    const step = Math.max(1, Math.floor(data.length / width));
    for (let x = 0; x < width; x += 1) {
      let peak = 0;
      for (let i = 0; i < step; i += 1) peak = Math.max(peak, Math.abs(data[x * step + i] ?? 0));
      const bar = peak * height * 0.9;
      context.fillRect(x, (height - bar) / 2, 1, bar);
    }
  }

  function preview() {
    if (!source) return;
    const audio = new Audio(URL.createObjectURL(source));
    audio.currentTime = start;
    audio.ontimeupdate = () => {
      if (audio.currentTime >= end) audio.pause();
    };
    audio.onended = () => URL.revokeObjectURL(audio.src);
    void audio.play();
  }

  function dragHandle(which, event) {
    event.preventDefault();
    const move = (moveEvent) => {
      if (!timeline) return;
      const bounds = timeline.getBoundingClientRect();
      const value = Math.max(0, Math.min(1, (moveEvent.clientX - bounds.left) / bounds.width)) * duration;
      if (which === 'start') start = Math.min(value, Math.max(0, end - 0.01));
      else end = Math.max(value, Math.min(duration, start + 0.01));
    };
    const stop = () => window.removeEventListener('pointermove', move);
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', stop, { once: true });
  }

  function useAudio() {
    if (!audioBuffer) return;
    const sampleRate = audioBuffer.sampleRate;
    const first = Math.floor(start * sampleRate);
    const last = Math.floor(end * sampleRate);
    const length = Math.max(1, last - first);
    const bytes = new ArrayBuffer(44 + length * 2);
    const view = new DataView(bytes);
    const write = (offset, value) => view.setUint32(offset, value, true);
    const write16 = (offset, value) => view.setUint16(offset, value, true);
    const writeText = (offset, value) =>
      [...value].forEach((char, index) => view.setUint8(offset + index, char.charCodeAt(0)));
    writeText(0, 'RIFF');
    write(4, 36 + length * 2);
    writeText(8, 'WAVE');
    writeText(12, 'fmt ');
    write(16, 16);
    write16(20, 1);
    write16(22, 1);
    write(24, sampleRate);
    write(28, sampleRate * 2);
    write16(32, 2);
    write16(34, 16);
    writeText(36, 'data');
    write(40, length * 2);
    const channel = audioBuffer.getChannelData(0);
    for (let index = 0; index < length; index += 1)
      write16(44 + index * 2, Math.max(-1, Math.min(1, channel[first + index] ?? 0)) * 0x7fff);
    onUse(new Blob([bytes], { type: 'audio/wav' }));
    onClose();
  }
</script>

{#if open}
  <div
    class="audio-modal-backdrop"
    role="presentation"
    onclick={(event) => event.target === event.currentTarget && onClose()}
  >
    <div class="audio-modal" role="dialog" aria-modal="true" aria-labelledby="editor-title">
      <div class="flex items-start justify-between gap-4">
        <div>
          <p class="text-xs font-black uppercase tracking-[0.18em] text-navy">Audio editor</p>
          <h2 id="editor-title" class="mt-1 text-xl font-black text-ink">Trim your take</h2>
        </div>
        <button class="icon-action" aria-label="Close editor" title="Close" onclick={onClose}>×</button>
      </div>
      {#if error}<p class="mt-4 rounded-xl bg-danger/10 p-3 text-sm font-bold text-danger">
          {error}
        </p>{:else}<div class="trim-timeline" bind:this={timeline} role="group" aria-label="Audio trim range">
          <canvas bind:this={canvas}></canvas>
          <div
            class="trim-handle trim-handle-start"
            style={`left: ${(start / duration) * 100}%`}
            role="slider"
            aria-label="Trim start"
            aria-valuenow={start}
            aria-valuemin="0"
            aria-valuemax={duration}
            tabindex="0"
            onpointerdown={(event) => dragHandle('start', event)}
          ></div>
          <div
            class="trim-handle trim-handle-end"
            style={`left: ${(end / duration) * 100}%`}
            role="slider"
            aria-label="Trim end"
            aria-valuenow={end}
            aria-valuemin="0"
            aria-valuemax={duration}
            tabindex="0"
            onpointerdown={(event) => dragHandle('end', event)}
          ></div>
        </div>
        <p class="mt-2 text-xs text-ink/60">
          {start.toFixed(2)}s – {end.toFixed(2)}s of {duration.toFixed(2)}s
        </p>{/if}
      <div class="mt-6 flex flex-wrap justify-end gap-2">
        <button
          class="action-button border-outline bg-white text-navy"
          disabled={!audioBuffer}
          onclick={() => {
            start = 0;
            end = duration;
          }}>Reset trim</button
        ><button
          class="action-button border-outline bg-white text-navy"
          disabled={!audioBuffer}
          onclick={preview}>▶ Preview</button
        ><button
          class="action-button bg-accent-blue text-white"
          disabled={!audioBuffer || end <= start}
          onclick={useAudio}>Use audio</button
        >
      </div>
    </div>
  </div>
{/if}
