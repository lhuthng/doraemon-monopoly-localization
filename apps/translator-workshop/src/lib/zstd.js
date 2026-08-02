import { compress as zstdCompress, decompress as zstdDecompress, init as zstdInit } from '@bokuweb/zstd-wasm';

export const WORK_ZSTD_LEVEL = 9;

let ready = null;

function ensureReady() {
  if (!ready) ready = zstdInit();
  return ready;
}

export async function compressWork(bytes) {
  await ensureReady();
  return zstdCompress(bytes, WORK_ZSTD_LEVEL);
}

export async function decompressWork(bytes) {
  await ensureReady();
  return zstdDecompress(bytes);
}
