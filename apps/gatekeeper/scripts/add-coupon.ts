import { createHash, randomBytes } from 'node:crypto';
import { activeHashes, recordCoupon } from './lib/coupon-registry.ts';
import { pushWorkerSecret, writeDevVar } from './lib/cloudflare.ts';

async function main() {
  const provided = process.argv.slice(2).filter(Boolean).join(' ');
  const coupon = provided || randomBytes(24).toString('base64url');
  if (coupon !== provided) {
    console.log('No coupon provided - generated a random one.');
  } else if (coupon.length < 8) {
    console.log('Warning: that coupon is very short and easy to guess.');
  }
  const hash = createHash('sha256').update(coupon).digest('hex');

  recordCoupon(coupon, hash);
  const active = activeHashes();
  const next = JSON.stringify(active);
  writeDevVar('COUPON_HASHES', next);

  console.log('COUPON   (share this with the contributor, never commit it):');
  console.log(coupon);
  console.log('');
  console.log(`Active coupons now (${active.length}):`);
  console.log(next);
  console.log('');

  await pushWorkerSecret('COUPON_HASHES', next);
  console.log('Pushed COUPON_HASHES to Cloudflare - the coupon is live.');
  console.log('The plaintext coupon is never sent; only its SHA-256 digest is stored server-side.');
}

main().catch((cause) => {
  console.error(cause instanceof Error ? cause.message : cause);
  process.exitCode = 1;
});
