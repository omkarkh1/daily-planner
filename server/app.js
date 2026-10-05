import fs from 'node:fs';
import fsp from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  AttemptLimiter,
  PASSWORD_MAX,
  PASSWORD_MIN,
  generateTotpSecret,
  hashPassword,
  hashToken,
  newToken,
  otpauthUri,
  safeEqual,
  verifyPassword,
  verifyTotp,
} from './auth.js';
import { COLORS } from './db.js';
import {
  HttpError,
  parseCookies,
  readJson,
  redirect,
  securityHeaders,
  sendJson,
  serializeCookie,
} from './http.js';

const PUBLIC_DIR = fileURLToPath(new URL('../public/', import.meta.url));
const COOKIE = 'cadence_sid';
const JSON_LIMIT = 64 * 1024;
const IMPORT_LIMIT = 20 * 1024 * 1024;
const NOTE_MAX = 10_000;
const TOUCH_INTERVAL = 60 * 60 * 1000;

const TYPES = {
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
  '.webmanifest': 'application/manifest+json',
  '.txt': 'text/plain; charset=utf-8',
};

// ---- validation ---------------------------------------------------------------
const bad = (message) => new HttpError(400, message);

function isDate(value) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value ?? '');
  if (!m) return false;
  const d = new Date(Date.UTC(+m[1], +m[2] - 1, +m[3]));
  return d.getUTCFullYear() === +m[1] && d.getUTCMonth() === +m[2] - 1 && d.getUTCDate() === +m[3];
}

const isMonth = (value) => /^\d{4}-(0[1-9]|1[0-2])$/.test(value ?? '');

function requireDate(value) {
  if (!isDate(value)) throw bad('Date must look like YYYY-MM-DD.');
  return value;
}

function requireId(value) {
  const id = Number(value);
  if (!Number.isSafeInteger(id) || id < 1) throw new HttpError(404, 'Not found.');
  return id;
}

function cleanName(value) {
  const name = typeof value === 'string' ? value.trim().replace(/\s+/g, ' ') : '';
  if (!name || name.length > 80) throw bad('Give your habit a name (up to 80 characters).');
  return name;
}

function cleanGoal(value) {
  const goal = Number(value);
  if (!Number.isInteger(goal) || goal < 1 || goal > 31) throw bad('Monthly goal must be a whole number from 1 to 31.');
  return goal;
}

function cleanColor(value) {
  if (!COLORS.includes(value)) throw bad('Unknown color.');
  return value;
}

function cleanNoteBody(value) {
  const body = typeof value === 'string' ? value.replace(/\r\n/g, '\n').trim() : '';
  if (!body) throw bad('Write something before saving your note.');
  if (body.length > NOTE_MAX) throw bad(`Notes can be up to ${NOTE_MAX.toLocaleString('en-US')} characters.`);
  return body;
}

function cleanUsername(value) {
  const username = typeof value === 'string' ? value.trim() : '';
  if (!username || username.length > 64) throw bad('Username must be 1–64 characters.');
  return username;
}

function cleanNewPassword(value) {
  if (typeof value !== 'string' || value.length < PASSWORD_MIN) {
    throw bad(`Password must be at least ${PASSWORD_MIN} characters.`);
  }
  if (value.length > PASSWORD_MAX) throw bad('Password is too long.');
  return value;
}

const str = (value, max = PASSWORD_MAX) => (typeof value === 'string' ? value.slice(0, max) : '');

function validateBackup(data) {
  if (!Array.isArray(data.habits) || !Array.isArray(data.notes)) {
    throw bad('That file does not look like a Cadence backup.');
  }
  const now = new Date().toISOString();
  const habits = data.habits.map((h, i) => {
    if (!h || typeof h !== 'object') throw bad(`Habit #${i + 1} is malformed.`);
    return {
      name: cleanName(h.name),
      goal: cleanGoal(h.goal),
      color: COLORS.includes(h.color) ? h.color : COLORS[i % 4],
      archived_at: typeof h.archived_at === 'string' ? h.archived_at : null,
      created_at: typeof h.created_at === 'string' ? h.created_at : now,
      checks: [...new Set(Array.isArray(h.checks) ? h.checks.filter(isDate) : [])],
    };
  });
  const notes = data.notes.map((n, i) => {
    if (!n || typeof n !== 'object' || !isDate(n.date)) throw bad(`Note #${i + 1} is malformed.`);
    return {
      date: n.date,
      body: cleanNoteBody(n.body),
      created_at: typeof n.created_at === 'string' ? n.created_at : now,
      updated_at: typeof n.updated_at === 'string' ? n.updated_at : now,
    };
  });
  return { habits, notes };
}

const publicHabit = (h) => ({
  id: h.id,
  name: h.name,
  goal: h.goal,
  color: h.color,
  position: h.position,
  archived: Boolean(h.archived_at),
  archivedAt: h.archived_at,
  createdAt: h.created_at,
});

const publicNote = (n) => ({ id: n.id, date: n.date, body: n.body, createdAt: n.created_at, updatedAt: n.updated_at });

// ---- app ----------------------------------------------------------------------
export async function createApp({ store, config, log = console }) {
  const loginLimiter = new AttemptLimiter({ max: 5, windowMs: 15 * 60_000, blockMs: 15 * 60_000 });
  const sensitiveLimiter = new AttemptLimiter({ max: 5, windowMs: 15 * 60_000, blockMs: 15 * 60_000 });
  // Compared against when the username is unknown so response time doesn't reveal it.
  const dummyHash = await hashPassword(newToken());
  const sessionMs = config.sessionDays * 24 * 60 * 60 * 1000;

  let setupToken = store.countUsers() === 0 ? config.setupToken || newToken().slice(0, 24) : null;

  const routes = [];
  const route = (method, pattern, handler, { auth = true, limit = JSON_LIMIT } = {}) => {
    const keys = [];
    const re = new RegExp(`^${pattern.replace(/:(\w+)/g, (_, k) => (keys.push(k), '([^/]+)'))}$`);
    routes.push({ method, re, keys, handler, auth, limit });
  };

  // ---- request helpers ----
  const clientIp = (req) => {
    if (config.trustProxy) {
      const forwarded = String(req.headers['x-forwarded-for'] || '')
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean);
      if (forwarded.length) return forwarded[forwarded.length - 1];
    }
    return req.socket.remoteAddress || 'unknown';
  };

  const isHttps = (req) =>
    Boolean(req.socket.encrypted) ||
    (config.trustProxy && String(req.headers['x-forwarded-proto'] || '').split(',')[0].trim() === 'https');

  const cookieSecure = (ctx) => config.cookieSecure ?? ctx.https;

  function setSessionCookie(ctx, token) {
    ctx.res.setHeader(
      'Set-Cookie',
      serializeCookie(COOKIE, token, { maxAge: sessionMs / 1000, secure: cookieSecure(ctx) }),
    );
  }

  function clearSessionCookie(ctx) {
    ctx.res.setHeader('Set-Cookie', serializeCookie(COOKIE, '', { maxAge: 0, secure: cookieSecure(ctx) }));
  }

  function startSession(ctx, user) {
    const token = newToken();
    store.purgeExpiredSessions();
    store.createSession({
      idHash: hashToken(token),
      userId: user.id,
      expiresAt: Date.now() + sessionMs,
      userAgent: str(ctx.req.headers['user-agent'], 300) || null,
      ip: ctx.ip,
    });
    setSessionCookie(ctx, token);
  }

  function authenticate(ctx) {
    const token = parseCookies(ctx.req.headers.cookie)[COOKIE];
    if (!token) return null;
    const idHash = hashToken(token);
    const session = store.getSession(idHash);
    const now = Date.now();
    if (!session) return null;
    if (session.expires_at <= now) {
      store.deleteSession(idHash);
      return null;
    }
    const user = store.getUserById(session.user_id);
    if (!user) return null;
    // Sliding expiry: active devices stay signed in.
    if (now - session.last_seen > TOUCH_INTERVAL) {
      store.touchSession(idHash, now, now + sessionMs);
      setSessionCookie(ctx, token);
    }
    return { user, session, idHash };
  }

  function guard(limiter, key) {
    const wait = limiter.retryAfter(key);
    if (wait > 0) {
      const minutes = Math.ceil(wait / 60);
      throw new HttpError(429, `Too many attempts. Take a breath and try again in ${minutes} minute${minutes === 1 ? '' : 's'}.`, {
        retryAfter: wait,
      });
    }
  }

  const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

  // ---- public auth routes ----
  route(
    'POST',
    '/api/setup',
    async (ctx) => {
      guard(loginLimiter, ctx.ip);
      if (store.countUsers() > 0 || !setupToken) throw new HttpError(409, 'An account already exists. Please sign in.');
      const { token, username, password } = ctx.body;
      if (!safeEqual(str(token).trim(), setupToken)) {
        loginLimiter.fail(ctx.ip);
        throw new HttpError(403, 'That setup token is not right. Copy it from the server log.');
      }
      const user = store.createUser(cleanUsername(username), await hashPassword(cleanNewPassword(password)));
      setupToken = null;
      loginLimiter.reset(ctx.ip);
      startSession(ctx, user);
      log.info?.(`Account "${user.username}" created.`);
      return { ok: true };
    },
    { auth: false },
  );

  route(
    'POST',
    '/api/login',
    async (ctx) => {
      guard(loginLimiter, ctx.ip);
      const username = str(ctx.body.username, 64).trim();
      const password = str(ctx.body.password);
      const code = str(ctx.body.code, 16);
      const user = username ? store.getUserByUsername(username) : null;
      const valid = (await verifyPassword(password, user?.password_hash ?? dummyHash)) && Boolean(user);
      if (!valid) {
        loginLimiter.fail(ctx.ip);
        await pause(250 + Math.random() * 250);
        throw new HttpError(401, 'That username and password don’t match. Give it another go.');
      }
      if (user.totp_secret) {
        if (!code) throw new HttpError(401, 'Enter the 6-digit code from your authenticator app.', { totpRequired: true });
        const counter = verifyTotp(user.totp_secret, code, user.totp_last_counter);
        if (counter === null) {
          loginLimiter.fail(ctx.ip);
          throw new HttpError(401, 'That code didn’t work. Use the newest code from your app.', { totpRequired: true });
        }
        store.setTotpCounter(user.id, counter);
      }
      loginLimiter.reset(ctx.ip);
      startSession(ctx, user);
      return { ok: true };
    },
    { auth: false },
  );

  route(
    'POST',
    '/api/logout',
    (ctx) => {
      const token = parseCookies(ctx.req.headers.cookie)[COOKIE];
      if (token) store.deleteSession(hashToken(token));
      clearSessionCookie(ctx);
      return { ok: true };
    },
    { auth: false },
  );

  // ---- account ----
  route('GET', '/api/me', (ctx) => ({
    username: ctx.auth.user.username,
    totpEnabled: Boolean(ctx.auth.user.totp_secret),
    sessionDays: config.sessionDays,
  }));

  route('POST', '/api/password', async (ctx) => {
    const key = `pw:${ctx.auth.user.id}`;
    guard(sensitiveLimiter, key);
    const { current, next } = ctx.body;
    if (!(await verifyPassword(str(current), ctx.auth.user.password_hash))) {
      sensitiveLimiter.fail(key);
      throw new HttpError(403, 'Your current password isn’t right.');
    }
    store.updatePassword(ctx.auth.user.id, await hashPassword(cleanNewPassword(next)));
    sensitiveLimiter.reset(key);
    const signedOut = store.deleteOtherSessions(ctx.auth.user.id, ctx.auth.idHash);
    return { ok: true, signedOut };
  });

  route('GET', '/api/sessions', (ctx) =>
    store.listSessions(ctx.auth.user.id).map((s) => ({
      current: s.id_hash === ctx.auth.idHash,
      createdAt: s.created_at,
      lastSeen: s.last_seen,
      userAgent: s.user_agent,
      ip: s.ip,
    })),
  );

  route('POST', '/api/sessions/revoke-others', (ctx) => ({
    signedOut: store.deleteOtherSessions(ctx.auth.user.id, ctx.auth.idHash),
  }));

  route('POST', '/api/2fa/setup', (ctx) => {
    const { user } = ctx.auth;
    if (user.totp_secret) throw new HttpError(409, 'Two-factor authentication is already on.');
    const secret = generateTotpSecret();
    store.setTotpPending(user.id, secret);
    return { secret, uri: otpauthUri(secret, user.username, config.appName) };
  });

  route('POST', '/api/2fa/enable', (ctx) => {
    const { user } = ctx.auth;
    const key = `2fa:${user.id}`;
    guard(sensitiveLimiter, key);
    if (!user.totp_pending) throw bad('Start the setup again to get a fresh key.');
    const counter = verifyTotp(user.totp_pending, ctx.body.code);
    if (counter === null) {
      sensitiveLimiter.fail(key);
      throw bad('That code didn’t match. Check the time on your phone and use the newest code.');
    }
    sensitiveLimiter.reset(key);
    store.enableTotp(user.id, user.totp_pending, counter);
    return { ok: true };
  });

  route('POST', '/api/2fa/disable', async (ctx) => {
    const { user } = ctx.auth;
    const key = `2fa:${user.id}`;
    guard(sensitiveLimiter, key);
    if (!user.totp_secret) return { ok: true };
    const passwordOk = await verifyPassword(str(ctx.body.password), user.password_hash);
    const counter = passwordOk ? verifyTotp(user.totp_secret, ctx.body.code, user.totp_last_counter) : null;
    if (counter === null) {
      sensitiveLimiter.fail(key);
      throw new HttpError(403, 'Password or code didn’t match.');
    }
    sensitiveLimiter.reset(key);
    store.disableTotp(user.id);
    return { ok: true };
  });

  // ---- habits & checks ----
  route('GET', '/api/month', (ctx) => {
    const month = ctx.query.get('month');
    if (!isMonth(month)) throw bad('Month must look like YYYY-MM.');
    const { habits, checks, notes } = store.getMonth(`${month}-01`, `${month}-31`);
    const byHabit = {};
    for (const c of checks) (byHabit[c.habit_id] ??= []).push(c.date);
    return { month, habits: habits.map(publicHabit), checks: byHabit, notes: notes.map(publicNote) };
  });

  route('GET', '/api/habits/archived', () => store.listArchived().map((h) => ({ ...publicHabit(h), totalChecks: h.total_checks })));

  route('POST', '/api/habits', (ctx) => {
    const { name, goal, color } = ctx.body;
    const habit = store.createHabit({
      name: cleanName(name),
      goal: cleanGoal(goal),
      color: color === undefined ? COLORS[store.countActiveHabits() % 4] : cleanColor(color),
    });
    return publicHabit(habit);
  });

  route('PATCH', '/api/habits/:id', (ctx) => {
    const id = requireId(ctx.params.id);
    const { name, goal, color, archived } = ctx.body;
    const habit = store.updateHabit(id, {
      name: name === undefined ? undefined : cleanName(name),
      goal: goal === undefined ? undefined : cleanGoal(goal),
      color: color === undefined ? undefined : cleanColor(color),
      archived: archived === undefined ? undefined : Boolean(archived),
    });
    if (!habit) throw new HttpError(404, 'Habit not found.');
    return publicHabit(habit);
  });

  route('DELETE', '/api/habits/:id', (ctx) => {
    if (!store.deleteHabit(requireId(ctx.params.id))) throw new HttpError(404, 'Habit not found.');
    return { ok: true };
  });

  route('POST', '/api/habits/reorder', (ctx) => {
    const { ids } = ctx.body;
    if (!Array.isArray(ids) || ids.length > 1000 || !ids.every((id) => Number.isSafeInteger(id) && id > 0)) {
      throw bad('Expected a list of habit ids.');
    }
    store.reorderHabits(ids);
    return { ok: true };
  });

  route('PUT', '/api/checks', (ctx) => {
    const habitId = requireId(ctx.body.habitId);
    const date = requireDate(ctx.body.date);
    if (!store.getHabit(habitId)) throw new HttpError(404, 'Habit not found.');
    store.setCheck(habitId, date, Boolean(ctx.body.done));
    return { ok: true };
  });

  // ---- notes ----
  route('POST', '/api/notes', (ctx) =>
    publicNote(store.createNote({ date: requireDate(ctx.body.date), body: cleanNoteBody(ctx.body.body) })),
  );

  route('PATCH', '/api/notes/:id', (ctx) => {
    const { date, body } = ctx.body;
    const note = store.updateNote(requireId(ctx.params.id), {
      date: date === undefined ? undefined : requireDate(date),
      body: body === undefined ? undefined : cleanNoteBody(body),
    });
    if (!note) throw new HttpError(404, 'Note not found.');
    return publicNote(note);
  });

  route('DELETE', '/api/notes/:id', (ctx) => {
    if (!store.deleteNote(requireId(ctx.params.id))) throw new HttpError(404, 'Note not found.');
    return { ok: true };
  });

  // ---- backup ----
  route('GET', '/api/export', (ctx) => {
    const today = new Date().toISOString().slice(0, 10);
    const body = JSON.stringify({ app: 'cadence', version: 1, exportedAt: new Date().toISOString(), ...store.exportAll() }, null, 2);
    ctx.res.writeHead(200, {
      'Content-Type': 'application/json; charset=utf-8',
      'Content-Disposition': `attachment; filename="cadence-backup-${today}.json"`,
      'Cache-Control': 'no-store',
      'Content-Length': Buffer.byteLength(body),
    });
    ctx.res.end(body);
  });

  route(
    'POST',
    '/api/import',
    (ctx) => {
      const backup = validateBackup(ctx.body);
      store.importAll(backup);
      return { ok: true, habits: backup.habits.length, notes: backup.notes.length };
    },
    { limit: IMPORT_LIMIT },
  );

  // ---- static files & pages ----
  async function serveFile(req, res, rel, cacheControl = 'no-cache') {
    const file = path.join(PUBLIC_DIR, rel);
    if (!file.startsWith(PUBLIC_DIR)) return false;
    let stat;
    try {
      stat = await fsp.stat(file);
    } catch {
      return false;
    }
    if (!stat.isFile()) return false;
    const etag = `W/"${stat.size.toString(16)}-${Math.floor(stat.mtimeMs).toString(16)}"`;
    const headers = { ETag: etag, 'Cache-Control': cacheControl };
    if (req.headers['if-none-match'] === etag) {
      res.writeHead(304, headers);
      res.end();
      return true;
    }
    const type = path.extname(file) === '.html' ? 'text/html; charset=utf-8' : TYPES[path.extname(file)];
    res.writeHead(200, { ...headers, 'Content-Type': type || 'application/octet-stream', 'Content-Length': stat.size });
    if (req.method === 'HEAD') res.end();
    else fs.createReadStream(file).pipe(res);
    return true;
  }

  async function handlePage(ctx) {
    const { req, res, url } = ctx;
    const page = url.pathname;
    if (page === '/healthz') {
      res.writeHead(200, { 'Content-Type': 'text/plain', 'Cache-Control': 'no-store' });
      return res.end('ok');
    }
    if (page === '/' || page === '/login' || page === '/setup') {
      const hasUser = store.countUsers() > 0;
      const auth = hasUser ? authenticate(ctx) : null;
      if (page === '/') {
        if (!hasUser) return redirect(res, '/setup');
        if (!auth) return redirect(res, '/login');
        return serveFile(req, res, 'app.html', 'no-store');
      }
      if (page === '/login') {
        if (!hasUser) return redirect(res, '/setup');
        if (auth) return redirect(res, '/');
        return serveFile(req, res, 'login.html', 'no-store');
      }
      if (hasUser) return redirect(res, auth ? '/' : '/login');
      return serveFile(req, res, 'setup.html', 'no-store');
    }
    let rel;
    try {
      rel = decodeURIComponent(page);
    } catch {
      rel = '';
    }
    if (rel && !rel.includes('\0') && !rel.endsWith('.html') && (await serveFile(req, res, rel.slice(1)))) return;
    res.writeHead(404, { 'Content-Type': 'text/plain; charset=utf-8' });
    res.end('Not found');
  }

  async function handleApi(ctx) {
    const { req, url } = ctx;
    let match;
    let allowed = false;
    for (const r of routes) {
      const m = r.re.exec(url.pathname);
      if (!m) continue;
      allowed = true;
      if (r.method !== req.method) continue;
      match = { route: r, params: Object.fromEntries(r.keys.map((k, i) => [k, decodeURIComponent(m[i + 1])])) };
      break;
    }
    if (!match) throw new HttpError(allowed ? 405 : 404, allowed ? 'Method not allowed.' : 'Not found.');

    if (req.method !== 'GET') {
      // CSRF: browsers only attach this header to same-origin fetches (a cross-site
      // request would need a CORS preflight we never grant), and SameSite=Lax
      // cookies already stay off cross-site form posts.
      const site = req.headers['sec-fetch-site'];
      if (req.headers['x-requested-with'] !== 'fetch' || (site && site !== 'same-origin' && site !== 'none')) {
        throw new HttpError(403, 'Cross-site request blocked.');
      }
    }

    ctx.params = match.params;
    if (match.route.auth) {
      ctx.auth = authenticate(ctx);
      if (!ctx.auth) throw new HttpError(401, 'Please sign in.');
    }
    ctx.body = ['POST', 'PUT', 'PATCH'].includes(req.method) ? await readJson(req, match.route.limit) : {};
    const result = await match.route.handler(ctx);
    if (!ctx.res.headersSent) sendJson(ctx.res, 200, result ?? { ok: true });
  }

  async function handle(req, res) {
    const url = new URL(req.url, 'http://localhost');
    const ctx = { req, res, url, query: url.searchParams, ip: clientIp(req), https: isHttps(req) };
    securityHeaders(res, { https: ctx.https });
    try {
      if (url.pathname.startsWith('/api/')) await handleApi(ctx);
      else if (req.method === 'GET' || req.method === 'HEAD') await handlePage(ctx);
      else throw new HttpError(405, 'Method not allowed.');
    } catch (err) {
      if (!(err instanceof HttpError)) log.error?.(err);
      const status = err instanceof HttpError ? err.status : 500;
      const message = err instanceof HttpError ? err.message : 'Something went wrong on our side.';
      if (res.headersSent) return res.end();
      const extra = err instanceof HttpError ? err.extra : {};
      const headers = extra.retryAfter ? { 'Retry-After': String(extra.retryAfter) } : {};
      if (status === 429 || status === 413) headers.Connection = 'close';
      sendJson(res, status, { error: message, ...extra }, headers);
    }
  }

  return {
    handle,
    get setupToken() {
      return setupToken;
    },
  };
}
