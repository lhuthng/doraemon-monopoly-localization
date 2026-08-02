import { activeHashes, loadRegistry } from './lib/coupon-registry.ts';

function shortHash(hash: string): string {
  return hash.slice(0, 12);
}

function main() {
  const entries = loadRegistry();
  const active = new Set(activeHashes());

  if (entries.length === 0) {
    console.log('No coupons recorded yet. Mint one with: make gatekeeper-add-coupon COUPON=...');
    return;
  }

  const activeCount = entries.filter((entry) => active.has(entry.hash)).length;
  console.log(`Coupons (${activeCount} active, ${entries.length - activeCount} revoked):`);
  console.log('');
  for (const entry of entries) {
    const status = active.has(entry.hash) ? 'ACTIVE' : 'revoked';
    console.log(`  [${status.padEnd(7)}] ${entry.coupon}  (${shortHash(entry.hash)}, ${entry.createdAt.slice(0, 10)})`);
  }
  console.log('');
  console.log('Revoke one with:   make gatekeeper-delete-coupon COUPON=... (or HASH=<sha256>)');
  console.log('Every change is pushed to Cloudflare automatically.');
}

main();
