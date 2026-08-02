import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { HeadObjectCommand, PutObjectCommand, S3Client } from '@aws-sdk/client-s3';

const accountId = process.env.R2_ACCOUNT_ID ?? '';
const accessKeyId = process.env.R2_ACCESS_KEY_ID ?? '';
const secretAccessKey = process.env.R2_SECRET_ACCESS_KEY ?? '';
const bucket = process.env.R2_BUCKET ?? 'doraemon-game-files';
const endpoint = process.env.R2_ENDPOINT ?? `https://${accountId}.r2.cloudflarestorage.com`;

if (!accountId || !accessKeyId || !secretAccessKey) {
  throw new Error(
    'Set R2_ACCOUNT_ID, R2_ACCESS_KEY_ID, and R2_SECRET_ACCESS_KEY. ' +
      'Create an R2 API token in the Cloudflare dashboard and add these to apps/resource-studio/.env (never committed).'
  );
}

const studio = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repository = resolve(studio, '../..');
const baseDir = resolve(repository, 'workspace', 'base');
const fingerprintsPath = resolve(repository, 'content', 'base-fingerprints.json');

function sha256(bytes: Buffer) {
  return createHash('sha256').update(bytes).digest('hex');
}

const client = new S3Client({ region: 'auto', endpoint, credentials: { accessKeyId, secretAccessKey } });

async function main() {
  const fingerprints: { files: Record<string, string> } = JSON.parse(await readFile(fingerprintsPath, 'utf8'));
  const files = Object.entries(fingerprints.files);

  for (const [name, expectedHash] of files) {
    const path = resolve(baseDir, name);
    const body = await readFile(path);
    const actualHash = sha256(body);
    if (actualHash !== expectedHash) {
      throw new Error(`Checksum mismatch for ${name}. Expected ${expectedHash}, got ${actualHash}.`);
    }
    await client.send(
      new PutObjectCommand({
        Bucket: bucket,
        Key: name,
        Body: body,
        ContentType: 'application/octet-stream',
        Metadata: { sha256: actualHash }
      })
    );
    console.log(`uploaded ${name} (${body.length.toLocaleString()} bytes)`);
  }

  for (const [name] of files) {
    const head = await client.send(new HeadObjectCommand({ Bucket: bucket, Key: name }));
    const storedHash = head.Metadata?.sha256;
    if (storedHash !== fingerprints.files[name]) {
      throw new Error(`Verification failed for ${name}: stored sha256 metadata is ${storedHash ?? 'missing'}.`);
    }
  }
  console.log(`\nAll ${files.length} files uploaded to r2://${bucket} and sha256 metadata verified.`);
}

main().catch((cause) => {
  console.error(cause instanceof Error ? cause.message : cause);
  process.exitCode = 1;
});
