import { createHash, randomBytes } from 'node:crypto';

function main() {
  const coupon = randomBytes(24).toString('base64url');
  const hash = createHash('sha256').update(coupon).digest('hex');
  console.log('Generated one new coupon (not activated).');
  console.log('');
  console.log('COUPON   (share this with the user, never commit it):');
  console.log(coupon);
  console.log('');
  console.log('HASH');
  console.log(hash);
  console.log('');
  console.log('To mint AND activate (recording it in the registry and pushing to');
  console.log('Cloudflare), use:');
  console.log('  make gatekeeper-add-coupon COUPON="' + coupon + '"');
}

main();
