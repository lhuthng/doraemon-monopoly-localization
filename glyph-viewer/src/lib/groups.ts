export type StringGroup = {
  id: string;
  label: string;
  detail: string;
};

// The archive only stores numeric paths. These names describe the decoded
// contents; the six dialogue tables deliberately remain unnamed characters.
export const STRING_GROUPS: StringGroup[] = [
  { id: '000', label: 'Global UI', detail: 'Prompts, menus, item names, and prototype notices' },
  { id: '001', label: 'Tools', detail: 'Tool and item descriptions' },
  { id: '002', label: 'Events', detail: 'Fortune, penalties, and game-event messages' },
  { id: '003', label: 'Dialogue A', detail: 'Parallel character reaction table' },
  { id: '004', label: 'Dialogue B', detail: 'Parallel character reaction table' },
  { id: '005', label: 'Dialogue C', detail: 'Parallel character reaction table' },
  { id: '006', label: 'Dialogue D', detail: 'Parallel character reaction table' },
  { id: '007', label: 'Dialogue E', detail: 'Parallel character reaction table' },
  { id: '008', label: 'Dialogue F', detail: 'Parallel character reaction table' }
];
