import { expect, test } from 'bun:test';
import { decodeCatalogue } from './catalogue-format.js';

test('ships a usable public dubbing catalogue', async () => {
  const bytes = new Uint8Array(
    await Bun.file(
      new URL('../../../resource-studio/src/lib/generated-dubbing-catalogue.zst', import.meta.url)
    ).arrayBuffer()
  );
  const catalogue = await decodeCatalogue(bytes);
  expect(catalogue.format).toBe('doraemon-monopoly-dubbing/v1');
  expect(catalogue.languages.english).toBeDefined();
  expect(catalogue.languages.vietnamese).toBeDefined();
  expect(catalogue.fingerprints.strings.length).toBeGreaterThan(0);
});
