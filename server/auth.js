import crypto from 'node:crypto';
import { promisify } from 'node:util';

const scrypt = promisify(crypto.scrypt);

// ---- passwords --------------------------------------------------------------
// Format: scrypt$N$r$p$salt$hash (base64url). Built on Node's crypto, no native deps.
const SCRYPT = { N: 2 ** 15, r: 8, p: 1, keylen: 64 };
const MAXMEM = 128 * 1024 * 1024;

export const PASSWORD_MIN = 8;
export const PASSWORD_MAX = 1024;

export async function hashPassword(password) {
  const salt = crypto.randomBytes(16);
  const hash = await scrypt(password.normalize('NFKC'), salt, SCRYPT.keylen, { ...SCRYPT, maxmem: MAXMEM });
  return ['scrypt', SCRYPT.N, SCRYPT.r, SCRYPT.p, salt.toString('base64url'), hash.toString('base64url')].join('$');
}

export async function verifyPassword(password, stored) {
  const [scheme, N, r, p, salt, hash] = String(stored).split('$');
  if (scheme !== 'scrypt' || !salt || !hash) return false;
  const expected = Buffer.from(hash, 'base64url');
  const actual = await scrypt(password.normalize('NFKC'), Buffer.from(salt, 'base64url'), expected.length, {
    N: Number(N),
    r: Number(r),
    p: Number(p),
    maxmem: MAXMEM,
  });
  return crypto.timingSafeEqual(actual, expected);
}

// ---- session tokens ---------------------------------------------------------
// The cookie carries a random token; only its SHA-256 is stored, so a leaked
// database cannot be replayed as a login.
export const newToken = () => crypto.randomBytes(32).toString('base64url');
export const hashToken = (token) => crypto.createHash('sha256').update(token).digest('hex');

export function safeEqual(a, b) {
  const x = Buffer.from(String(a));
  const y = Buffer.from(String(b));
  return x.length === y.length && crypto.timingSafeEqual(x, y);
}

// ---- TOTP (RFC 6238) ----------------------------------------------------------
const B32 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';

export function base32Encode(buf) {
  let bits = 0;
  let value = 0;
  let out = '';
  for (const byte of buf) {
    value = (value << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      out += B32[(value >>> (bits - 5)) & 31];
      bits -= 5;
    }
  }
  if (bits > 0) out += B32[(value << (5 - bits)) & 31];
  return out;
}

export function base32Decode(str) {
  const clean = str.toUpperCase().replace(/[\s=-]/g, '');
  let bits = 0;
  let value = 0;
  const out = [];
  for (const ch of clean) {
    const idx = B32.indexOf(ch);
    if (idx === -1) throw new Error('Invalid base32');
    value = (value << 5) | idx;
    bits += 5;
    if (bits >= 8) {
      out.push((value >>> (bits - 8)) & 255);
      bits -= 8;
    }
  }
  return Buffer.from(out);
}

export const generateTotpSecret = () => base32Encode(crypto.randomBytes(20));

export function totpCode(secret, counter, digits = 6) {
  const msg = Buffer.alloc(8);
  msg.writeBigUInt64BE(BigInt(counter));
  const hmac = crypto.createHmac('sha1', base32Decode(secret)).update(msg).digest();
  const offset = hmac[hmac.length - 1] & 0xf;
  const code = (hmac.readUInt32BE(offset) & 0x7fffffff) % 10 ** digits;
  return String(code).padStart(digits, '0');
}

/** Returns the matched time-step counter, or null. Codes at or before lastCounter are rejected (no replay). */
export function verifyTotp(secret, code, lastCounter = -1, now = Date.now()) {
  const clean = String(code ?? '').replace(/\s/g, '');
  if (!/^\d{6}$/.test(clean)) return null;
  const current = Math.floor(now / 30000);
  for (const counter of [current - 1, current, current + 1]) {
    if (counter <= lastCounter) continue;
    if (safeEqual(totpCode(secret, counter), clean)) return counter;
  }
  return null;
}

export function otpauthUri(secret, account, issuer) {
  const label = encodeURIComponent(`${issuer}:${account}`);
  const params = new URLSearchParams({ secret, issuer, algorithm: 'SHA1', digits: '6', period: '30' });
  return `otpauth://totp/${label}?${params}`;
}

// ---- brute-force protection ---------------------------------------------------
export class AttemptLimiter {
  constructor({ max = 5, windowMs = 15 * 60_000, blockMs = 15 * 60_000 } = {}) {
    Object.assign(this, { max, windowMs, blockMs });
    this.entries = new Map();
  }

  /** Seconds until the key may try again (0 = allowed). */
  retryAfter(key) {
    const entry = this.entries.get(key);
    if (!entry?.blockedUntil) return 0;
    const ms = entry.blockedUntil - Date.now();
    if (ms <= 0) {
      this.entries.delete(key);
      return 0;
    }
    return Math.ceil(ms / 1000);
  }

  fail(key) {
    const now = Date.now();
    let entry = this.entries.get(key);
    if (!entry || now - entry.first > this.windowMs) entry = { count: 0, first: now, blockedUntil: 0 };
    entry.count += 1;
    if (entry.count >= this.max) entry.blockedUntil = now + this.blockMs;
    this.entries.set(key, entry);
    if (this.entries.size > 10_000) this.prune(now);
  }

  reset(key) {
    this.entries.delete(key);
  }

  prune(now = Date.now()) {
    for (const [key, entry] of this.entries) {
      if (entry.blockedUntil < now && now - entry.first > this.windowMs) this.entries.delete(key);
    }
  }
}
