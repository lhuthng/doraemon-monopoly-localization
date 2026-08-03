import { compress as zstdCompress, decompress as zstdDecompress, init as zstdInit } from '@bokuweb/zstd-wasm';

export const WORK_ZSTD_LEVEL = 9;

let ready = null;

function ensureReady() {
  if (!ready) ready = zstdInit();
  return ready;
}

export async function compressZstd(bytes, level = WORK_ZSTD_LEVEL) {
  await ensureReady();
  return zstdCompress(bytes, level);
}

export async function decompressZstd(bytes) {
  await ensureReady();
  return zstdDecompress(bytes);
}

export const compressWork = (bytes) => compressZstd(bytes, WORK_ZSTD_LEVEL);
export const decompressWork = (bytes) => decompressZstd(bytes);
