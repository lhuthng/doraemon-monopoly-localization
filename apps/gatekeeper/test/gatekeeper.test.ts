import { describe, expect, test } from 'bun:test';
import {
  checkCoupon,
  checkMaintainerSecret,
  handleRequest,
  parseCouponHashes,
  resolveFile,
  safeEqual,
  sha256Hex,
  type Env
} from '../src/index.ts';

function kvMock() {
  const store = new Map<string, string>();
  return {
    async get(key: string, type?: string) {
      const value = store.get(key);
      if (type === 'json') return value ? JSON.parse(value) : null;
      return value ?? null;
    },
    async put(key: string, value: string, options?: { expirationTtl?: number }) {
      store.set(key, value);
      void options;
    }
  };
}

function bucketMock(name: string, hash: string, etag = 'etag-1') {
  return {
    async get(key: string) {
      if (key !== name) return null;
      return { body: `contents-of-${name}`, httpEtag: etag, customMetadata: { sha256: hash } };
    }
  };
}

const COUPON = 'test-coupon-abcdef';
const COUPON_HASH = '51f93440562066327d5fd9853132244d24c5bbdc334fb300535c3de2a3e430d1';

function makeEnv(overrides: Partial<Env> = {}): Env {
  const kv = kvMock();
  return {
    GAME_FILES: bucketMock('strings.dat', 'expected-hash'),
    LIMITS: kv,
    MAINTAINER_SECRET: 'maintainer-secret',
    COUPON_HASHES: JSON.stringify([COUPON_HASH]),
    ALLOWED_ORIGINS: '',
    ...overrides
  } as unknown as Env;
}

describe('sha256Hex', () => {
  test('matches known vector', async () => {
    expect(await sha256Hex('abc')).toBe('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
  });
});

describe('safeEqual', () => {
  test('equal strings match', () => expect(safeEqual('abc', 'abc')).toBe(true));
  test('length mismatch fails', () => expect(safeEqual('abc', 'abcd')).toBe(false));
  test('same-length mismatch fails', () => expect(safeEqual('abc', 'abd')).toBe(false));
});

describe('parseCouponHashes', () => {
  test('parses a JSON array', () => expect(parseCouponHashes('["a","b"]')).toEqual(['a', 'b']));
  test('rejects malformed JSON', () => expect(parseCouponHashes('not-json')).toEqual([]));
  test('rejects non-arrays', () => expect(parseCouponHashes('"a"')).toEqual([]));
  test('empty value yields empty list', () => expect(parseCouponHashes(undefined)).toEqual([]));
});

describe('checkCoupon', () => {
  test('accepts the hashed coupon', async () => {
    const request = new Request('https://gatekeeper/api/files', { headers: { 'X-Coupon': COUPON } });
    expect(await checkCoupon(request, makeEnv())).toBe(true);
  });
  test('rejects a wrong coupon', async () => {
    const request = new Request('https://gatekeeper/api/files', { headers: { 'X-Coupon': 'wrong' } });
    expect(await checkCoupon(request, makeEnv())).toBe(false);
  });
  test('rejects when no hashes configured', async () => {
    const request = new Request('https://gatekeeper/api/files', { headers: { 'X-Coupon': COUPON } });
    expect(await checkCoupon(request, makeEnv({ COUPON_HASHES: '' }))).toBe(false);
  });
});

describe('checkMaintainerSecret', () => {
  test('accepts the bearer token', async () => {
    const request = new Request('https://gatekeeper/api/files', {
      headers: { Authorization: 'Bearer maintainer-secret' }
    });
    expect(await checkMaintainerSecret(request, makeEnv())).toBe(true);
  });
  test('rejects a wrong token', async () => {
    const request = new Request('https://gatekeeper/api/files', {
      headers: { Authorization: 'Bearer nope' }
    });
    expect(await checkMaintainerSecret(request, makeEnv())).toBe(false);
  });
  test('rejects a missing header', async () => {
    expect(await checkMaintainerSecret(new Request('https://gatekeeper/api/files'), makeEnv())).toBe(false);
  });
});

describe('resolveFile', () => {
  test('accepts an allowed file', () => {
    expect(resolveFile(new Request('https://gatekeeper/api/files?name=voice.dat'))).toBe('voice.dat');
  });
  test('rejects a disallowed name', () => {
    expect(resolveFile(new Request('https://gatekeeper/api/files?name=secret.txt'))).toBeNull();
  });
  test('rejects path traversal', () => {
    expect(resolveFile(new Request('https://gatekeeper/api/files?name=../.env'))).toBeNull();
  });
  test('rejects a missing name', () => {
    expect(resolveFile(new Request('https://gatekeeper/api/files'))).toBeNull();
  });
});

describe('handleRequest', () => {
  test('health endpoint works without auth', async () => {
    const response = await handleRequest(new Request('https://gatekeeper/api/health'), makeEnv());
    expect(response.status).toBe(200);
  });

  test('serves a file with a valid coupon', async () => {
    const request = new Request('https://gatekeeper/api/files?name=strings.dat', {
      headers: { 'X-Coupon': COUPON }
    });
    const response = await handleRequest(request, makeEnv());
    expect(response.status).toBe(200);
    expect(response.headers.get('X-SHA256')).toBe('expected-hash');
    expect(await response.text()).toBe('contents-of-strings.dat');
  });

  test('serves a file with the maintainer secret', async () => {
    const request = new Request('https://gatekeeper/api/files?name=strings.dat', {
      headers: { Authorization: 'Bearer maintainer-secret' }
    });
    const response = await handleRequest(request, makeEnv());
    expect(response.status).toBe(200);
  });

  test('returns 401 without valid credentials', async () => {
    const response = await handleRequest(new Request('https://gatekeeper/api/files?name=strings.dat'), makeEnv());
    expect(response.status).toBe(401);
  });

  test('returns 400 for an unsupported file', async () => {
    const request = new Request('https://gatekeeper/api/files?name=hack.txt', {
      headers: { Authorization: 'Bearer maintainer-secret' }
    });
    const response = await handleRequest(request, makeEnv());
    expect(response.status).toBe(400);
  });

  test('returns 404 when the object is missing', async () => {
    const env = makeEnv({ GAME_FILES: bucketMock('strings.dat', 'x') as unknown as Env['GAME_FILES'] });
    const request = new Request('https://gatekeeper/api/files?name=voice.dat', {
      headers: { Authorization: 'Bearer maintainer-secret' }
    });
    const response = await handleRequest(request, env);
    expect(response.status).toBe(404);
  });

  test('returns 404 for an unknown route', async () => {
    const response = await handleRequest(new Request('https://gatekeeper/other'), makeEnv());
    expect(response.status).toBe(404);
  });

  test('rate limits repeated requests from one IP', async () => {
    const env = makeEnv();
    for (let index = 0; index < 20; index += 1) {
      const request = new Request('https://gatekeeper/api/files?name=strings.dat', {
        headers: { 'CF-Connecting-IP': '1.2.3.4' }
      });
      await handleRequest(request, env);
    }
    const request = new Request('https://gatekeeper/api/files?name=strings.dat', {
      headers: { 'CF-Connecting-IP': '1.2.3.4' }
    });
    const response = await handleRequest(request, env);
    expect(response.status).toBe(429);
  });

  test('preflight passes allowed origins', async () => {
    const env = makeEnv({ ALLOWED_ORIGINS: 'https://workshop.example' });
    const response = await handleRequest(
      new Request('https://gatekeeper/api/files', {
        method: 'OPTIONS',
        headers: { Origin: 'https://workshop.example' }
      }),
      env
    );
    expect(response.status).toBe(204);
    expect(response.headers.get('Access-Control-Allow-Origin')).toBe('https://workshop.example');
  });

  test('rejects a disallowed origin', async () => {
    const env = makeEnv({ ALLOWED_ORIGINS: 'https://workshop.example' });
    const response = await handleRequest(
      new Request('https://gatekeeper/api/files?name=strings.dat', {
        headers: { Origin: 'https://evil.example', Authorization: 'Bearer maintainer-secret' }
      }),
      env
    );
    expect(response.status).toBe(403);
  });
});
