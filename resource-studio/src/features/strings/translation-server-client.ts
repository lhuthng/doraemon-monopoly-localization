export async function translateLine(model: string, target: string, text: string) {
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), 120_000);
  let response: Response;
  try {
    response = await fetch('/api/translate', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ model, target, texts: [text] }),
      signal: controller.signal
    });
  } catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError')
      throw new Error('Translation server timed out after 120 seconds.', { cause: error });
    throw new Error(
      `Cannot reach translation server: ${error instanceof Error ? error.message : String(error)}`,
      {
        cause: error
      }
    );
  } finally {
    window.clearTimeout(timeout);
  }
  const payload = await response.json();
  if (!response.ok) throw new Error(payload?.error || `Translation server returned HTTP ${response.status}.`);
  const translated = payload?.translations?.[0];
  if (typeof translated !== 'string') throw new Error('Translation server returned no translation text.');
  return translated;
}

export async function prepareModel(model: string, onStatus: (message: string) => void) {
  const warmup = await fetch('/api/warmup', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ model })
  });
  if (!warmup.ok && warmup.status !== 202) {
    const payload = await warmup.json().catch(() => ({}));
    throw new Error(payload?.error || `Translation server warmup returned HTTP ${warmup.status}.`);
  }
  const started = Date.now();
  while (Date.now() - started < 15 * 60_000) {
    const response = await fetch('/api/status');
    if (!response.ok) throw new Error(`Translation server status returned HTTP ${response.status}.`);
    const payload = await response.json();
    const state = payload?.models?.[model];
    if (state?.state === 'ready') return;
    if (state?.state === 'error')
      throw new Error(state.message || 'Translation server failed to load the model.');
    onStatus(state?.message || 'Waiting for the translation server…');
    await new Promise((resolve) => window.setTimeout(resolve, 750));
  }
  throw new Error('Translation server did not become ready within 15 minutes.');
}
