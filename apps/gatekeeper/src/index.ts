export interface Env {
  GAME_FILES: R2Bucket;
  LIMITS: KVNamespace;
  MAINTAINER_SECRET: string;
  COUPON_HASHES: string;
  ALLOWED_ORIGINS: string;
}

export const ALLOWED_FILES = [
  'Doraemon.exe',
  'strings.dat',
  'sysfont.dat',
  'Sprite1.dat',
  'sprite2.dat',
  'bitmaps.dat',
  'voice.dat'
];

const RATE_LIMIT_MAX = 20;
const RATE_LIMIT_WINDOW_SECONDS = 60;

export async function sha256Bytes(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('');
}

export function sha256Hex(input: string): Promise<string> {
  return sha256Bytes(new TextEncoder().encode(input));
}

export function safeEqual(left: string, right: string): boolean {
  if (left.length !== right.length) return false;
  let diff = 0;
  for (let index = 0; index < left.length; index += 1) {
    diff |= left.charCodeAt(index) ^ right.charCodeAt(index);
  }
  return diff === 0;
}

export function parseCouponHashes(raw: string | undefined): string[] {
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((hash): hash is string => typeof hash === 'string') : [];
  } catch {
    return [];
  }
}

export function originAllowed(request: Request, env: Env): boolean {
  const origin = request.headers.get('Origin');
  if (!origin) return true;
  const allowed = (env.ALLOWED_ORIGINS ?? '')
    .split(',')
    .map((value) => value.trim())
    .filter(Boolean);
  return allowed.length === 0 || allowed.includes(origin);
}

function corsHeaders(request: Request, env: Env): Record<string, string> {
  const origin = request.headers.get('Origin');
  const headers: Record<string, string> = {
    'Access-Control-Allow-Methods': 'GET, HEAD, PUT, OPTIONS',
    'Access-Control-Allow-Headers': 'Authorization, X-Coupon, Content-Type',
    'Access-Control-Expose-Headers': 'X-Uploaded-At, X-SHA256, ETag',
    'Access-Control-Max-Age': '86400',
    'Cache-Control': 'no-store',
    Vary: 'Origin'
  };
  if (origin && originAllowed(request, env)) headers['Access-Control-Allow-Origin'] = origin;
  return headers;
}

async function applyRateLimit(ip: string | null, env: Env): Promise<boolean> {
  if (!ip || !env.LIMITS) return false;
  const minute = Math.floor(Date.now() / (RATE_LIMIT_WINDOW_SECONDS * 1000));
  const key = `rate:${ip}:${minute}`;
  const current = Number((await env.LIMITS.get(key, 'text')) ?? '0');
  if (current >= RATE_LIMIT_MAX) return true;
  await env.LIMITS.put(key, String(current + 1), { expirationTtl: RATE_LIMIT_WINDOW_SECONDS * 2 });
  return false;
}

function clientIp(request: Request): string | null {
  return request.headers.get('CF-Connecting-IP') ?? request.headers.get('X-Forwarded-For')?.split(',')[0] ?? null;
}

export async function checkMaintainerSecret(request: Request, env: Env): Promise<boolean> {
  const auth = request.headers.get('Authorization');
  if (!auth?.startsWith('Bearer ')) return false;
  const token = auth.slice('Bearer '.length).trim();
  if (!token || !env.MAINTAINER_SECRET) return false;
  return safeEqual(token, env.MAINTAINER_SECRET);
}

export async function checkCoupon(request: Request, env: Env): Promise<boolean> {
  const coupon = request.headers.get('X-Coupon')?.trim();
  if (!coupon) return false;
  const hashes = parseCouponHashes(env.COUPON_HASHES);
  if (!hashes.length) return false;
  const hash = await sha256Hex(coupon);
  return hashes.some((candidate) => safeEqual(hash, candidate));
}

export function resolveFile(request: Request): string | null {
  const name = new URL(request.url).searchParams.get('name');
  if (!name) return null;
  return ALLOWED_FILES.includes(name) ? name : null;
}

/** Derives the R2 key for a caller's saved work blob. The worker never inspects the blob itself. */
export async function resolveWorkKey(request: Request, env: Env): Promise<string | null> {
  if (await checkMaintainerSecret(request, env)) return 'work/maintainer.zst';
  const coupon = request.headers.get('X-Coupon')?.trim();
  if (!coupon) return null;
  const hashes = parseCouponHashes(env.COUPON_HASHES);
  if (!hashes.length) return null;
  const hash = await sha256Hex(coupon);
  if (!hashes.some((candidate) => safeEqual(hash, candidate))) return null;
  return `work/coupon/${hash}.zst`;
}

function jsonError(cors: Record<string, string>, status: number, message: string): Response {
  return new Response(JSON.stringify({ error: message }), {
    status,
    headers: { ...cors, 'Content-Type': 'application/json' }
  });
}

async function handleWork(request: Request, env: Env, cors: Record<string, string>): Promise<Response> {
  const key = await resolveWorkKey(request, env);
  if (!key) {
    return jsonError(cors, 401, 'Unauthorized.');
  }

  if (request.method === 'PUT') {
    const bytes = new Uint8Array(await request.arrayBuffer());
    const sha256 = await sha256Bytes(bytes);
    const uploadedAt = new Date().toISOString();
    await env.GAME_FILES.put(key, bytes, {
      customMetadata: { sha256, uploadedAt },
      httpMetadata: { contentType: 'application/octet-stream' }
    });
    return new Response(JSON.stringify({ uploadedAt, size: bytes.length, sha256 }), {
      status: 200,
      headers: { ...cors, 'Content-Type': 'application/json' }
    });
  }

  const object = request.method === 'HEAD' ? await env.GAME_FILES.head(key) : await env.GAME_FILES.get(key);
  if (!object) {
    return jsonError(cors, 404, 'No saved work yet.');
  }

  const headers: Record<string, string> = {
    ...cors,
    'Content-Type': 'application/octet-stream',
    'Cache-Control': 'no-store',
    'X-Uploaded-At': object.uploaded.toISOString(),
    'Last-Modified': object.uploaded.toUTCString(),
    'Content-Length': String(object.size)
  };
  if (object.httpEtag) headers.ETag = object.httpEtag;
  const storedHash = object.customMetadata?.sha256;
  if (typeof storedHash === 'string') headers['X-SHA256'] = storedHash;

  if (request.method === 'HEAD') {
    return new Response(null, { status: 200, headers });
  }
  return new Response((object as R2ObjectBody).body, { status: 200, headers });
}

export async function handleRequest(request: Request, env: Env): Promise<Response> {
  const cors = corsHeaders(request, env);
  const url = new URL(request.url);

  if (request.method === 'OPTIONS') {
    return new Response(null, { status: 204, headers: cors });
  }
  if (!['GET', 'HEAD', 'PUT'].includes(request.method)) {
    return new Response(JSON.stringify({ error: 'Method not allowed.' }), {
      status: 405,
      headers: { ...cors, 'Content-Type': 'application/json' }
    });
  }
  if (!originAllowed(request, env)) {
    return new Response(JSON.stringify({ error: 'Origin not allowed.' }), {
      status: 403,
      headers: { ...cors, 'Content-Type': 'application/json' }
    });
  }

  if (url.pathname === '/api/health') {
    return new Response(JSON.stringify({ ok: true }), {
      status: 200,
      headers: { ...cors, 'Content-Type': 'application/json' }
    });
  }

  if (url.pathname === '/api/work') {
    if (await applyRateLimit(clientIp(request), env)) {
      return new Response(JSON.stringify({ error: 'Too many requests. Try again shortly.' }), {
        status: 429,
        headers: { ...cors, 'Content-Type': 'application/json', 'Retry-After': String(RATE_LIMIT_WINDOW_SECONDS) }
      });
    }
    return handleWork(request, env, cors);
  }

  if (url.pathname !== '/api/files') {
    return new Response(JSON.stringify({ error: 'Not found.' }), {
      status: 404,
      headers: { ...cors, 'Content-Type': 'application/json' }
    });
  }

  if (request.method !== 'GET') {
    return jsonError(cors, 405, 'Method not allowed.');
  }

  if (await applyRateLimit(clientIp(request), env)) {
    return new Response(JSON.stringify({ error: 'Too many requests. Try again shortly.' }), {
      status: 429,
      headers: { ...cors, 'Content-Type': 'application/json', 'Retry-After': String(RATE_LIMIT_WINDOW_SECONDS) }
    });
  }

  const file = resolveFile(request);
  if (!file) {
    return jsonError(cors, 400, 'Unknown or unsupported file.');
  }

  const authenticated = (await checkMaintainerSecret(request, env)) || (await checkCoupon(request, env));
  if (!authenticated) {
    return jsonError(cors, 401, 'Unauthorized.');
  }

  const object = await env.GAME_FILES.get(file);
  if (!object) {
    return jsonError(cors, 404, 'File is not available yet.');
  }

  const headers: Record<string, string> = {
    ...cors,
    'Content-Type': 'application/octet-stream',
    'Cache-Control': 'no-store'
  };
  if (object.httpEtag) headers.ETag = object.httpEtag;
  const storedHash = object.customMetadata?.sha256;
  if (typeof storedHash === 'string') headers['X-SHA256'] = storedHash;
  return new Response(object.body, { status: 200, headers });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    return handleRequest(request, env);
  }
};
