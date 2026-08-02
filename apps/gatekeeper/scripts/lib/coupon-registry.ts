import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export interface CouponEntry {
  coupon: string;
  hash: string;
  createdAt: string;
  revoked?: boolean;
}

const registryPath = resolve(dirname(fileURLToPath(import.meta.url)), '../..', 'coupons.registry.json');

export function loadRegistry(): CouponEntry[] {
  try {
    const parsed = JSON.parse(readFileSync(registryPath, 'utf8'));
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function saveRegistry(entries: CouponEntry[]): void {
  writeFileSync(registryPath, JSON.stringify(entries, null, 2) + '\n');
}

export function recordCoupon(coupon: string, hash: string): void {
  const entries = loadRegistry();
  const existing = entries.find((entry) => entry.hash === hash);
  if (existing) {
    existing.revoked = false;
  } else {
    entries.push({ coupon, hash, createdAt: new Date().toISOString() });
  }
  saveRegistry(entries);
}

export function revokeCoupon(hash: string): boolean {
  const entries = loadRegistry();
  let found = false;
  for (const entry of entries) {
    if (entry.hash === hash && !entry.revoked) {
      entry.revoked = true;
      found = true;
    }
  }
  if (found) saveRegistry(entries);
  return found;
}

export function findByHash(hash: string): CouponEntry | undefined {
  return loadRegistry().find((entry) => entry.hash === hash);
}

export function activeHashes(): string[] {
  return loadRegistry()
    .filter((entry) => !entry.revoked)
    .map((entry) => entry.hash);
}
