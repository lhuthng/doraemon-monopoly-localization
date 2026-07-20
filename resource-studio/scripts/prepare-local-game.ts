import { copyFile, mkdir, readFile, readdir, stat } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const language = process.argv[2];
const gameFolder = process.argv[3];
const force = process.argv.slice(4).includes('--force');
const files = ['strings.dat', 'sysfont.dat', 'Sprite1.dat', 'sprite2.dat', 'bitmaps.dat', 'voice.dat'];

if (language !== 'english' && language !== 'vietnamese') {
  throw new Error('Choose a workspace: english or vietnamese.');
}
if (!gameFolder) {
  throw new Error(`Usage: bun run setup-${language === 'english' ? 'en' : 'vi'} /path/to/game [--force]`);
}

const studio = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repository = resolve(studio, '..');
const source = resolve(gameFolder);
const localGame = resolve(studio, 'local-game');
const origin = resolve(localGame, 'origin');
const target = resolve(localGame, language);
const payload = resolve(repository, 'patches', `${language}.dmpatch`);

async function exists(path: string) {
  try {
    await stat(path);
    return true;
  } catch {
    return false;
  }
}

function mapPairs(available: Set<string>) {
  return [...available]
    .map((file) => /^map(\d{4})\.dat$/i.exec(file)?.[1])
    .filter((suffix): suffix is string => Boolean(suffix))
    .filter((suffix) => available.has(`mapElem${suffix}.dat`))
    .sort()
    .flatMap((suffix) => [`map${suffix}.dat`, `mapElem${suffix}.dat`]);
}

for (const file of files) {
  if (!(await exists(resolve(source, file)))) {
    throw new Error(`The selected game folder is missing ${file}. No files were created.`);
  }
}
const mapFiles = mapPairs(new Set(await readdir(source)));
if (!(await exists(payload))) {
  throw new Error(
    `Missing ${payload}. This language has no tracked resource payload yet, so it cannot be prepared.`
  );
}

const existing = await Promise.all(files.map((file) => exists(resolve(target, file))));
if (existing.some(Boolean) && !force) {
  throw new Error(
    `${target} already contains a workspace. Refusing to overwrite your edits. Add --force to rebuild it from the selected game folder.`
  );
}

await mkdir(origin, { recursive: true });
const originalStrings = resolve(origin, 'strings.dat');
const sourceStrings = resolve(source, 'strings.dat');
if (await exists(originalStrings)) {
  if (!(await readFile(originalStrings)).equals(await readFile(sourceStrings))) {
    throw new Error(
      `${originalStrings} belongs to a different original game. Move it aside before preparing this workspace.`
    );
  }
} else {
  await copyFile(sourceStrings, originalStrings);
}
const originalVoice = resolve(origin, 'voice.dat');
const sourceVoice = resolve(source, 'voice.dat');
if (await exists(originalVoice)) {
  if (!(await readFile(originalVoice)).equals(await readFile(sourceVoice))) {
    throw new Error(
      `${originalVoice} belongs to a different original game. Move it aside before preparing this workspace.`
    );
  }
} else {
  await copyFile(sourceVoice, originalVoice);
}

await mkdir(target, { recursive: true });
const child = Bun.spawn(
  [
    'cargo',
    'run',
    '-p',
    'patch-build',
    '--',
    'materialize',
    '--payload',
    payload,
    '--base-dir',
    source,
    '--output-dir',
    target
  ],
  { cwd: repository, stdout: 'inherit', stderr: 'inherit' }
);
if ((await child.exited) !== 0) {
  throw new Error('Resource preparation failed. Check the error above before using the workspace.');
}
// Map resources are presently inspect-only and are absent from translation
// payloads, so materialization does not create them in the language workspace.
await Promise.all(mapFiles.map((file) => copyFile(resolve(source, file), resolve(target, file))));

console.log(
  `Prepared ${language} resources in ${target}. Run bun run dev-${language === 'english' ? 'en' : 'vi'}.`
);
