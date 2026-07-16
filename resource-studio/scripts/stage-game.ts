import { copyFile, mkdir, readdir, rm } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const language = process.argv[2];
if (language !== 'english' && language !== 'vietnamese') {
  throw new Error('Choose a workspace: english or vietnamese.');
}

const studio = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const source = resolve(studio, 'local-game', language);
const destination = resolve(studio, 'public', 'game');
const files = ['strings.dat', 'sysfont.dat', 'Sprite1.dat', 'sprite2.dat', 'bitmaps.dat'];

let available: Set<string>;
try {
  available = new Set(await readdir(source));
} catch {
  throw new Error(
    `No local ${language} workspace was found at ${source}. Copy your own game files there first; this repository cannot provide copyrighted game data.`
  );
}

const missing = files.filter((file) => !available.has(file));
if (missing.length > 0) {
  throw new Error(
    `The ${language} workspace is incomplete (${missing.join(', ')}). Add the files from your own game installation before starting the Studio.`
  );
}

await mkdir(destination, { recursive: true });
for (const entry of await readdir(destination)) {
  if (entry !== '.gitkeep') await rm(resolve(destination, entry), { recursive: true });
}
for (const file of files) await copyFile(resolve(source, file), resolve(destination, file));

console.log(`Loaded the ${language} workspace into public/game. Starting Resource Studio…`);
