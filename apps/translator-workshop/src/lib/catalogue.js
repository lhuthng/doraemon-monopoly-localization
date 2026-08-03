import catalogueUrl from '../../../resource-studio/src/lib/generated-dubbing-catalogue.zst?url';
import { decodeCatalogue } from './catalogue-format.js';

let cached = null;

export async function loadCatalogue() {
  if (cached) return cached;
  const response = await fetch(catalogueUrl);
  if (!response.ok) {
    throw new Error(`Dubbing catalogue unavailable (HTTP ${response.status}).`);
  }
  cached = await decodeCatalogue(new Uint8Array(await response.arrayBuffer()));
  return cached;
}
