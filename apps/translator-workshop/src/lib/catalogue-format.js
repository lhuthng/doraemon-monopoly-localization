import { decompressZstd } from './zstd.js';

export async function decodeCatalogue(bytes) {
  const decoded = await decompressZstd(bytes);
  return JSON.parse(new TextDecoder().decode(decoded));
}
