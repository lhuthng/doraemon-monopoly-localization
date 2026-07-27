export const ownerLabels = {
  doraemon: 'Doraemon',
  nobita: 'Nobita',
  dorami: 'Dorami',
  shizuka: 'Shizuka',
  suneo: 'Suneo',
  gian: 'Gian'
};

export const ownerIcons = {
  doraemon: './game-assets/characters/doraemon.png',
  nobita: './game-assets/characters/nobita.png',
  dorami: './game-assets/characters/dorami.png',
  shizuka: './game-assets/characters/shizuka.png',
  suneo: './game-assets/characters/suneo.png',
  gian: './game-assets/characters/gian.png'
};

export const ownerSmallIcons = Object.fromEntries(
  Object.entries(ownerIcons).map(([owner, icon]) => [owner, icon.replace('.png', '-s.png')])
);

export function gadgetAsset(assetId) {
  return `./game-assets/gadgets/${assetId}.png`;
}
