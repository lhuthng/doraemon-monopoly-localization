import { describe, expect, test } from 'bun:test';
import { dialogueVoicePath } from './voice';

describe('dialogue voice mapping', () => {
  test('maps version 1.26 Cantonese bank 0 without inventing later voices', () => {
    const banks = Array.from({ length: 6 }, () => [84, 64, 42]);
    expect(dialogueVoicePath(3, 0, banks)).toEqual([0, 0, 0]);
    expect(dialogueVoicePath(8, 83, banks)).toEqual([5, 0, 83]);
    expect(dialogueVoicePath(3, 84, banks)).toBeUndefined();
  });

  test('maps the sparse version 1.18 Chinese bank-3 continuation', () => {
    const banks = Array.from({ length: 6 }, () => [89, 64, 42, 37]);
    expect(dialogueVoicePath(3, 88, banks)).toEqual([0, 0, 88]);
    expect(dialogueVoicePath(3, 89, banks)).toEqual([0, 3, 0]);
    expect(dialogueVoicePath(4, 100, banks)).toEqual([1, 3, 11]);
    expect(dialogueVoicePath(3, 101, banks)).toBeUndefined();
    expect(dialogueVoicePath(3, 102, banks)).toEqual([0, 3, 12]);
    expect(dialogueVoicePath(3, 103, banks)).toBeUndefined();
    expect(dialogueVoicePath(3, 104, banks)).toEqual([0, 3, 13]);
    expect(dialogueVoicePath(3, 124, banks)).toEqual([0, 3, 33]);
    expect(dialogueVoicePath(3, 125, banks)).toBeUndefined();
    expect(dialogueVoicePath(3, 126, banks)).toBeUndefined();
    expect(dialogueVoicePath(3, 127, banks)).toEqual([0, 3, 34]);
    expect(dialogueVoicePath(3, 128, banks)).toEqual([0, 3, 35]);
    expect(dialogueVoicePath(3, 129, banks)).toBeUndefined();
    expect(dialogueVoicePath(8, 130, banks)).toEqual([5, 3, 36]);
  });
});
