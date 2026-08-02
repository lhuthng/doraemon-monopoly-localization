import { activeHashes, findByHash, loadRegistry, revokeCoupon } from './lib/coupon-registry.ts';
import { pushWorkerSecret, writeDevVar } from './lib/cloudflare.ts';

async function main() {
  const provided = process.argv.slice(2).filter(Boolean).join(' ');
  if (!provided) {
    console.error('Usage: bun run delete-coupon <COUPON|HASH>');
    console.error('  COUPON: the plaintext coupon (looked up in the registry)');
    console.error('  HASH:   a full 64-hex SHA-256 (for coupons not in the registry)');
    process.exitCode = 1;
    return;
  }

  const isHash = /^[0-9a-f]{64}$/.test(provided);
  let hash: string;
  if (isHash) {
    hash = provided;
  } else {
    const entry = loadRegistry().find((entry) => entry.coupon === provided);
    if (!entry) {
      console.error(`Coupon "${provided}" is not in the local registry.`);
      console.error('Pass its full SHA-256 hash instead, or minted coupons are recorded in coupons.registry.json.');
      process.exitCode = 1;
      return;
    }
    hash = entry.hash;
  }

  const revoked = revokeCoupon(hash);
  const entry = findByHash(hash);
  const active = activeHashes();
  const next = JSON.stringify(active);
  writeDevVar('COUPON_HASHES', next);

  const label = entry ? entry.coupon : hash.slice(0, 12);
  if (revoked) {
    console.log(`Revoked ${label}.`);
  } else {
    console.log(`${label} was not active — nothing changed in the active set.`);
  }
  console.log(`Active coupons now (${active.length}):`);
  console.log(next);
  console.log('');

  await pushWorkerSecret('COUPON_HASHES', next);
  console.log('Pushed the updated COUPON_HASHES to Cloudflare — the coupon no longer works.');
}

main().catch((cause) => {
  console.error(cause instanceof Error ? cause.message : cause);
  process.exitCode = 1;
});
