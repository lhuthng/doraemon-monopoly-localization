import { compressWork, decompressWork } from './zstd.js';

export const COUPON_KEY = 'doraemon-monopoly-translator/coupon';

export function saveCoupon(coupon) {
  localStorage.setItem(COUPON_KEY, (coupon ?? '').trim());
}

export function savedCoupon() {
  return localStorage.getItem(COUPON_KEY) ?? '';
}

async function assertOk(response) {
  if (response.ok) return response;
  const body = await response.text().catch(() => '');
  throw new Error(`HTTP ${response.status}${body ? `: ${body}` : ''}`);
}

function couponHeaders(coupon) {
  return { 'X-Coupon': (coupon ?? '').trim() };
}

export async function cloudWorkMeta(gatekeeperUrl, coupon) {
  const response = await fetch(`${gatekeeperUrl}/api/work`, {
    method: 'HEAD',
    headers: couponHeaders(coupon)
  });
  if (response.status === 404) return null;
  await assertOk(response);
  const uploadedAt = response.headers.get('X-Uploaded-At');
  const size = Number(response.headers.get('Content-Length') ?? 0);
  return {
    uploadedAt: uploadedAt ? new Date(uploadedAt).getTime() : null,
    size: Number.isFinite(size) ? size : 0
  };
}

export async function saveCloudWork(gatekeeperUrl, coupon, zipBytes) {
  const compressed = await compressWork(zipBytes);
  const response = await fetch(`${gatekeeperUrl}/api/work`, {
    method: 'PUT',
    headers: { ...couponHeaders(coupon), 'Content-Type': 'application/octet-stream' },
    body: compressed
  });
  await assertOk(response);
  const payload = await response.json().catch(() => ({}));
  return {
    uploadedAt: payload.uploadedAt ? new Date(payload.uploadedAt).getTime() : Date.now()
  };
}

export async function loadCloudWork(gatekeeperUrl, coupon) {
  const response = await assertOk(
    await fetch(`${gatekeeperUrl}/api/work`, { headers: couponHeaders(coupon) })
  );
  const compressed = new Uint8Array(await response.arrayBuffer());
  return decompressWork(compressed);
}
