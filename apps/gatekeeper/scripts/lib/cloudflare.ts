import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const WORKER_NAME = 'doraemon-gatekeeper';

const gatekeeperDir = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const devVarsPath = resolve(gatekeeperDir, '.dev.vars');
const tfvarsPath = resolve(gatekeeperDir, 'terraform', 'terraform.tfvars');

export function resolveApiToken(): string | null {
  return process.env.CLOUDFLARE_API_TOKEN ?? null;
}

export function resolveAccountId(): string | null {
  if (process.env.CLOUDFLARE_ACCOUNT_ID) return process.env.CLOUDFLARE_ACCOUNT_ID;
  try {
    const match = /^account_id\s*=\s*"([^"]+)"$/m.exec(readFileSync(tfvarsPath, 'utf8'));
    if (match) return match[1];
  } catch {
    // terraform.tfvars may not exist yet
  }
  try {
    const match = /^ACCOUNT_ID=(.*)$/m.exec(readFileSync(devVarsPath, 'utf8'));
    if (match?.[1]) return match[1];
  } catch {
    // .dev.vars may not exist yet
  }
  return null;
}

export function assertCredentials(): { token: string; accountId: string } {
  const token = resolveApiToken();
  const accountId = resolveAccountId();
  if (!token || !accountId) {
    throw new Error(
      'Cloudflare credentials not found. Put them in apps/gatekeeper/.env ' +
        '(copy apps/gatekeeper/.env.example — it is never committed):\n' +
        '  CLOUDFLARE_API_TOKEN=...\n  CLOUDFLARE_ACCOUNT_ID=...\n' +
        'or export CLOUDFLARE_API_TOKEN / CLOUDFLARE_ACCOUNT_ID in your shell.'
    );
  }
  return { token, accountId };
}

export function readDevVar(name: string): string | null {
  try {
    const match = new RegExp(`^${name}=(.*)$`, 'm').exec(readFileSync(devVarsPath, 'utf8'));
    return match?.[1] ?? null;
  } catch {
    return null;
  }
}

export function writeDevVar(name: string, value: string): void {
  const text = readFileSync(devVarsPath, 'utf8');
  const pattern = new RegExp(`^${name}=.*$`, 'm');
  const updated = pattern.test(text)
    ? text.replace(pattern, `${name}=${value}`)
    : `${text.replace(/\n?$/, '\n')}${name}=${value}\n`;
  writeFileSync(devVarsPath, updated);
}

export async function pushWorkerSecret(name: string, text: string): Promise<void> {
  const { token, accountId } = assertCredentials();
  const response = await fetch(
    `https://api.cloudflare.com/client/v4/accounts/${accountId}/workers/scripts/${WORKER_NAME}/secrets`,
    {
      method: 'PUT',
      headers: {
        Authorization: `Bearer ${token}`,
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({ name, text, type: 'secret_text' })
    }
  );
  const payload = (await response.json().catch(() => ({}))) as {
    success?: boolean;
    errors?: { message?: string }[];
  };
  if (!response.ok || !payload?.success) {
    const message = payload?.errors?.map((error) => error.message).join('; ') ?? `HTTP ${response.status}`;
    throw new Error(`Failed to push secret ${name}: ${message}`);
  }
}
