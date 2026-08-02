import { activeHashes } from './lib/coupon-registry.ts';
import { pushWorkerSecret, writeDevVar } from './lib/cloudflare.ts';

async function main() {
  const active = activeHashes();
  const next = JSON.stringify(active);
  writeDevVar('COUPON_HASHES', next);

  console.log(`Active coupons (${active.length}):`);
  console.log(next);
  console.log('');

  await pushWorkerSecret('COUPON_HASHES', next);
  console.log('Pushed COUPON_HASHES to Cloudflare. The registry and the worker secret now match.');
}

main().catch((cause) => {
  console.error(cause instanceof Error ? cause.message : cause);
  process.exitCode = 1;
});
