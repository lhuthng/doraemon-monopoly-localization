import { expect, test } from 'bun:test';
import catalogue from './catalogue.js';

test('ships a usable public dubbing catalogue', () => {
  expect(catalogue.format).toBe('doraemon-monopoly-dubbing/v1');
  expect(catalogue.languages.english).toBeDefined();
  expect(catalogue.languages.vietnamese).toBeDefined();
  expect(catalogue.fingerprints.strings.length).toBeGreaterThan(0);
});
