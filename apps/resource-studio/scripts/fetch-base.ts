import { createHash } from 'node:crypto';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const force = process.argv.slice(2).includes('--force');
const gatekeeperUrl = (process.env.CLOUDFLARE_GATEKEEPER_URL ?? '').replace(/\/$/, '');
const secret = process.env.CLOUDFLARE_GATEKEEPER_SECRET ?? '';

if (!gatekeeperUrl || !secret) {
  throw new Error(
    'Set CLOUDFLARE_GATEKEEPER_URL and CLOUDFLARE_GATEKEEPER_SECRET. ' +
      'Add them to apps/resource-studio/.env (never committed).'
  );
}

const studio = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repository = resolve(studio, '../..');
const baseDir = resolve(repository, 'workspace', 'base');
const fingerprintsPath = resolve(repository, 'content', 'base-fingerprints.json');

async function exists(path: string) {
  try {
    await stat(path);
    return true;
  } catch {
    return false;
  }
}

function sha256(bytes: Buffer) {
  return createHash('sha256').update(bytes).digest('hex');
}

async function main() {
  const fingerprints: { files: Record<string, string> } = JSON.parse(await readFile(fingerprintsPath, 'utf8'));
  const files = Object.entries(fingerprints.files);

  await mkdir(baseDir, { recursive: true });
  let fetched = 0;
  let skipped = 0;

  for (const [name, expectedHash] of files) {
    const target = resolve(baseDir, name);
    if (await exists(target)) {
      const existingHash = sha256(await readFile(target));
      if (existingHash === expectedHash) {
        skipped += 1;
        console.log(`skip  ${name} (already present and valid)`);
        continue;
      }
      if (!force) {
        throw new Error(
          `${target} exists and differs from the expected original. Move it aside or pass --force to overwrite.`
        );
      }
    }

    const response = await fetch(`${gatekeeperUrl}/api/files?name=${encodeURIComponent(name)}`, {
      headers: { Authorization: `Bearer ${secret}` }
    });
    if (!response.ok) {
      throw new Error(`Failed to fetch ${name}: HTTP ${response.status} ${await response.text()}`);
    }
    const bytes = Buffer.from(await response.arrayBuffer());
    const actualHash = sha256(bytes);
    if (actualHash !== expectedHash) {
      throw new Error(`Checksum mismatch for ${name}. Expected ${expectedHash}, got ${actualHash}.`);
    }
    await writeFile(target, bytes);
    fetched += 1;
    console.log(`ok    ${name} (${bytes.length.toLocaleString()} bytes, sha256 verified)`);
  }

  console.log(`\nFetched ${fetched}, skipped ${skipped} into workspace/base/.`);
  if (fetched) {
    console.log('Run `make prepare` to build the Studio workspaces from these files.');
  }
}

main().catch((cause) => {
  console.error(cause instanceof Error ? cause.message : cause);
  process.exitCode = 1;
});
