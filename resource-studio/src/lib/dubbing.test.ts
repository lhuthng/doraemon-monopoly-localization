import { describe, expect, test } from 'bun:test';
import {
  compareRecordIds,
  gadgetMetadataForStringId,
  gadgetVoiceId,
  normalizeDialogueFile,
  ownerForStringId,
  voiceIdFromFilename
} from './dubbing';

describe('dubbing source format', () => {
  test('assigns dialogue groups to their fixed owners', () => {
    expect(ownerForStringId('003/000')).toBe('doraemon');
    expect(ownerForStringId('008/123')).toBe('gian');
    expect(ownerForStringId('001/003')).toBe('others');
  });

  test('normalizes and numerically sorts dialogue records', () => {
    expect(
      normalizeDialogueFile('doraemon', {
        records: [
          { id: '003/010', translation: '  Ten  ' },
          { id: '003/002', translation: 'Two' },
          { id: '003/003', translation: '  ' }
        ]
      }).records
    ).toEqual([
      { id: '003/002', translation: 'Two' },
      { id: '003/010', translation: '  Ten  ' }
    ]);
    expect(compareRecordIds('003/010', '003/002')).toBeGreaterThan(0);
  });

  test('rejects records in the wrong owner file and parses canonical WAV names', () => {
    expect(() =>
      normalizeDialogueFile('nobita', { records: [{ id: '003/000', translation: 'No' }] })
    ).toThrow();
    expect(voiceIdFromFilename('003-001-011.wav')).toBe('003/001/011');
    expect(voiceIdFromFilename('voice.wav')).toBeUndefined();
  });

  test('maps all gadget records and their six character voice slots canonically', () => {
    for (let slot = 0; slot <= 41; slot += 1) {
      const id = `001/${String(slot).padStart(3, '0')}`;
      expect(gadgetMetadataForStringId(id)).toEqual({ assetId: slot, voiceSlot: slot });
      for (let character = 0; character < 6; character += 1)
        expect(gadgetVoiceId(character, slot)).toBe(
          `${String(character).padStart(3, '0')}/002/${String(slot).padStart(3, '0')}`
        );
    }
    expect(gadgetMetadataForStringId('001/042')).toBeUndefined();
    expect(gadgetMetadataForStringId('003/000')).toBeUndefined();
  });
});
