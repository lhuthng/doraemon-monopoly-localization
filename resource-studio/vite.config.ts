import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig, type Plugin } from 'vite';
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import {
  DUBBING_FORMAT,
  DUBBING_OWNERS,
  isDubbingLanguage,
  normalizeDialogueFile,
  ownerForVoiceId
} from './src/lib/dubbing';
import { parseWav, VOICE_SAMPLE_RATE } from './src/lib/voice-formats';

const execute = promisify(execFile);
const studio = resolve(dirname(fileURLToPath(import.meta.url)));
const repository = resolve(studio, '..');

function respond(response: import('node:http').ServerResponse, status: number, body: unknown) {
  response.statusCode = status;
  response.setHeader('Content-Type', 'application/json');
  response.end(JSON.stringify(body));
}

async function requestBody(request: import('node:http').IncomingMessage) {
  const chunks: Buffer[] = [];
  for await (const chunk of request) chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  return JSON.parse(Buffer.concat(chunks).toString('utf8')) as {
    language?: string;
    translations?: Record<string, string>;
    voices?: { id: string; wav: string | null }[];
  };
}

function dubbingBridge(): Plugin {
  return {
    name: 'doraemon-dubbing-bridge',
    configureServer(server) {
      server.middlewares.use('/__dubbing', async (request, response) => {
        if (request.method !== 'POST') return respond(response, 405, { error: 'POST required.' });
        try {
          const body = await requestBody(request);
          if (!body.language || !isDubbingLanguage(body.language))
            throw new Error('Invalid dubbing language.');
          const language = body.language;
          if (request.url === '/sync') {
            await execute('bun', ['scripts/dubbing.ts', 'sync', language], { cwd: studio });
            await execute('bun', ['scripts/stage-game.ts', language], { cwd: studio });
            return respond(response, 200, { status: `Synced dubbing/${language}; reload the Studio.` });
          }
          if (request.url === '/check') {
            await execute('bun', ['scripts/dubbing.ts', 'check', language], { cwd: studio });
            return respond(response, 200, { status: `dubbing/${language} is valid.` });
          }
          if (request.url !== '/save') return respond(response, 404, { error: 'Unknown dubbing action.' });
          const root = resolve(repository, 'dubbing', language);
          for (const owner of DUBBING_OWNERS) {
            const records: { id: string; translation: string }[] = [];
            for (const [id, translation] of Object.entries(body.translations ?? {})) {
              if (!translation.trim()) continue;
              try {
                const normalized = normalizeDialogueFile(owner, { records: [{ id, translation }] });
                if (normalized.records.length) records.push(normalized.records[0]);
              } catch {
                // This record belongs to another owner file.
              }
            }
            const path = resolve(root, 'dialogue', `${owner}.json`);
            await mkdir(dirname(path), { recursive: true });
            await writeFile(path, `${JSON.stringify(normalizeDialogueFile(owner, { records }), null, 2)}\n`);
          }
          for (const voice of body.voices ?? []) {
            const owner = ownerForVoiceId(voice.id);
            if (!owner) throw new Error(`Invalid voice ID ${voice.id}.`);
            const path = resolve(root, 'voices', owner, `${voice.id.replaceAll('/', '-')}.wav`);
            if (voice.wav === null) {
              await rm(path, { force: true });
              continue;
            }
            const wav = Buffer.from(voice.wav, 'base64');
            const info = parseWav(wav);
            if (
              info.format !== 1 ||
              info.channels !== 1 ||
              info.sampleRate !== VOICE_SAMPLE_RATE ||
              info.bitsPerSample !== 16
            )
              throw new Error(`${voice.id} must be mono 22.05 kHz 16-bit PCM WAV.`);
            await mkdir(dirname(path), { recursive: true });
            await writeFile(path, wav);
          }
          const manifest = resolve(root, 'manifest.json');
          try {
            await readFile(manifest);
          } catch {
            await mkdir(root, { recursive: true });
            await writeFile(manifest, `${JSON.stringify({ format: DUBBING_FORMAT, language }, null, 2)}\n`);
          }
          await execute('bun', ['scripts/dubbing.ts', 'organize', language], { cwd: studio });
          return respond(response, 200, { status: `Saved Studio changes to dubbing/${language}.` });
        } catch (error) {
          return respond(response, 400, { error: error instanceof Error ? error.message : String(error) });
        }
      });
    }
  };
}

export default defineConfig(({ command }) => ({
  base: '/',
  build: { copyPublicDir: false },
  server: { proxy: { '/api': 'http://127.0.0.1:5184' } },
  plugins: [svelte(), ...(command === 'serve' ? [dubbingBridge()] : [])]
}));
