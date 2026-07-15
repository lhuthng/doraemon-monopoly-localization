export type StringGroup = {
  id: string;
  label: string;
  detail: string;
};

// The archive only stores numeric paths. These names describe the decoded
// contents and the six parallel character dialogue tables.
export const STRING_GROUPS: StringGroup[] = [
  { id: '000', label: 'Global', detail: 'Prompts, menus, item names, and prototype notices' },
  { id: '001', label: 'Gadgets', detail: 'Gadget names and descriptions' },
  { id: '002', label: 'Events', detail: 'Fortune, penalties, and game-event messages' },
  { id: '003', label: 'Dialogues — Doraemon', detail: 'Doraemon character dialogue table' },
  { id: '004', label: 'Dialogues — Nobita', detail: 'Nobita character dialogue table' },
  { id: '005', label: 'Dialogues — Dorami', detail: 'Dorami character dialogue table' },
  { id: '006', label: 'Dialogues — Shizuka', detail: 'Shizuka character dialogue table' },
  { id: '007', label: 'Dialogues — Suneo', detail: 'Suneo character dialogue table' },
  { id: '008', label: 'Dialogues — Gian', detail: 'Gian character dialogue table' }
];
