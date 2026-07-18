import { copyFile, mkdir, readdir, rm } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const language = process.argv[2];
if (language !== 'english' && language !== 'vietnamese') {
  throw new Error('Choose a workspace: english or vietnamese.');
}

const studio = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const originSource = resolve(studio, 'local-game', 'origin');
const languageSource = resolve(studio, 'local-game', language);
const destination = resolve(studio, 'public', 'game');
const files = ['strings.dat', 'sysfont.dat', 'Sprite1.dat', 'sprite2.dat', 'bitmaps.dat'];

let originAvailable: Set<string>;
try {
  originAvailable = new Set(await readdir(originSource));
} catch {
  throw new Error(
    `No origin workspace was found at ${originSource}. Copy your own original game files there first; this repository cannot provide copyrighted game data.`
  );
}

if (!originAvailable.has('strings.dat')) {
  throw new Error(
    `The origin workspace is missing strings.dat. Add the original game file before starting the Studio.`
  );
}

let languageAvailable: Set<string>;
try {
  languageAvailable = new Set(await readdir(languageSource));
} catch {
  throw new Error(
    `No local ${language} workspace was found at ${languageSource}. Copy your own game files there first; this repository cannot provide copyrighted game data.`
  );
}

const missing = files.filter((file) => !languageAvailable.has(file));
if (missing.length > 0) {
  throw new Error(
    `The ${language} workspace is incomplete (${missing.join(', ')}). Add the files from your own game installation before starting the Studio.`
  );
}

await mkdir(destination, { recursive: true });
for (const entry of await readdir(destination)) {
  if (entry !== '.gitkeep') await rm(resolve(destination, entry), { recursive: true });
}

await copyFile(resolve(originSource, 'strings.dat'), resolve(destination, 'strings-origin.dat'));
for (const file of files) await copyFile(resolve(languageSource, file), resolve(destination, file));

const prepare = Bun.spawn(['bun', 'scripts/prepare-graphics.ts'], {
  cwd: studio,
  stdout: 'inherit',
  stderr: 'inherit'
});
if ((await prepare.exited) !== 0)
  throw new Error('Graphics pre-preparation failed. The Studio was not started.');

console.log(`Loaded and pre-prepared the ${language} workspace. Starting Resource Studio…`);
