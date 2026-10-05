import assert from 'node:assert/strict';
import http from 'node:http';
import { after, before, describe, test } from 'node:test';
import { createApp } from '../server/app.js';
import { totpCode, verifyTotp, hashPassword, verifyPassword, base32Encode } from '../server/auth.js';
import { loadConfig } from '../server/config.js';
import { openDb } from '../server/db.js';

const quiet = { info() {}, error() {} };

async function startServer(env = {}) {
  const config = loadConfig({ DB_PATH: ':memory:', SETUP_TOKEN: 'test-setup-token', ...env });
  const store = openDb(config.dbPath);
  const app = await createApp({ store, config, log: quiet });
  const server = http.createServer(app.handle);
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  return { base, store, app, close: () => new Promise((r) => server.close(r)) };
}

/** Minimal cookie-jar client. */
function client(base) {
  let cookie = '';
  const request = async (method, path, body, headers = {}) => {
    const res = await fetch(base + path, {
      method,
      redirect: 'manual',
      headers: {
        ...(method !== 'GET' ? { 'X-Requested-With': 'fetch' } : {}),
        ...(body !== undefined ? { 'Content-Type': 'application/json' } : {}),
        ...(cookie ? { Cookie: cookie } : {}),
        ...headers,
      },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const set = res.headers.get('set-cookie');
    if (set) cookie = set.split(';')[0].endsWith('=') ? '' : set.split(';')[0];
    const type = res.headers.get('content-type') || '';
    const data = type.includes('json') ? await res.json() : await res.text();
    return { status: res.status, data, headers: res.headers };
  };
  return {
    get: (p, h) => request('GET', p, undefined, h),
    post: (p, b, h) => request('POST', p, b, h),
    put: (p, b) => request('PUT', p, b),
    patch: (p, b) => request('PATCH', p, b),
    del: (p) => request('DELETE', p),
    get cookie() {
      return cookie;
    },
  };
}

describe('crypto helpers', () => {
  test('TOTP matches the RFC 6238 SHA-1 test vector', () => {
    const secret = base32Encode(Buffer.from('12345678901234567890'));
    // T = 59s -> counter 1 -> 94287082 (8 digits) -> 287082 (6 digits)
    assert.equal(totpCode(secret, 1), '287082');
    assert.equal(verifyTotp(secret, '287082', -1, 59_000), 1);
    assert.equal(verifyTotp(secret, '287082', 1, 59_000), null, 'replayed code is rejected');
    assert.equal(verifyTotp(secret, '12345', -1, 59_000), null);
  });

  test('passwords hash with a salt and verify', async () => {
    const a = await hashPassword('correct horse battery');
    const b = await hashPassword('correct horse battery');
    assert.notEqual(a, b);
    assert.ok(await verifyPassword('correct horse battery', a));
    assert.ok(!(await verifyPassword('wrong horse battery', a)));
  });
});

describe('first-run setup and authentication', () => {
  let srv;
  before(async () => (srv = await startServer()));
  after(() => srv.close());

  test('pages redirect to setup until an account exists', async () => {
    const c = client(srv.base);
    assert.equal((await c.get('/')).headers.get('location'), '/setup');
    assert.equal((await c.get('/login')).headers.get('location'), '/setup');
    assert.equal((await c.get('/setup')).status, 200);
  });

  test('setup requires the token printed in the log', async () => {
    const c = client(srv.base);
    const wrong = await c.post('/api/setup', { token: 'nope', username: 'me', password: 'longenough' });
    assert.equal(wrong.status, 403);
    const short = await c.post('/api/setup', { token: 'test-setup-token', username: 'me', password: 'short' });
    assert.equal(short.status, 400);
    const ok = await c.post('/api/setup', { token: 'test-setup-token', username: 'owner', password: 'a strong passphrase' });
    assert.equal(ok.status, 200);
    assert.match(ok.headers.get('set-cookie'), /HttpOnly/);
    assert.match(ok.headers.get('set-cookie'), /SameSite=Lax/);
    assert.equal((await c.get('/api/me')).data.username, 'owner');
    const again = await client(srv.base).post('/api/setup', { token: 'test-setup-token', username: 'x', password: 'another one!' });
    assert.equal(again.status, 409, 'setup only works once');
  });

  test('protected routes require a session', async () => {
    const c = client(srv.base);
    assert.equal((await c.get('/api/me')).status, 401);
    assert.equal((await c.get('/api/month?month=2026-10')).status, 401);
    assert.equal((await c.get('/')).headers.get('location'), '/login');
    assert.equal((await c.get('/app.html')).status, 404, 'pages are only reachable through their routes');
    assert.equal((await c.get('/login')).status, 200);
  });

  test('login, logout, and wrong credentials', async () => {
    const c = client(srv.base);
    assert.equal((await c.post('/api/login', { username: 'owner', password: 'nope nope' })).status, 401);
    assert.equal((await c.post('/api/login', { username: 'ghost', password: 'a strong passphrase' })).status, 401);
    const ok = await c.post('/api/login', { username: 'OWNER', password: 'a strong passphrase' });
    assert.equal(ok.status, 200, 'username is case-insensitive');
    assert.equal((await c.get('/')).status, 200);
    assert.equal((await c.get('/login')).headers.get('location'), '/');
    await c.post('/api/logout');
    assert.equal((await c.get('/api/me')).status, 401);
  });

  test('state-changing requests need the same-origin header', async () => {
    const c = client(srv.base);
    await c.post('/api/login', { username: 'owner', password: 'a strong passphrase' });
    const noHeader = await fetch(`${srv.base}/api/habits`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Cookie: c.cookie },
      body: JSON.stringify({ name: 'x', goal: 3 }),
    });
    assert.equal(noHeader.status, 403);
    const crossSite = await c.post('/api/habits', { name: 'x', goal: 3 }, { 'Sec-Fetch-Site': 'cross-site' });
    assert.equal(crossSite.status, 403);
  });

  test('security headers are set', async () => {
    const res = await client(srv.base).get('/login');
    assert.match(res.headers.get('content-security-policy'), /default-src 'self'/);
    assert.equal(res.headers.get('x-frame-options'), 'DENY');
    assert.equal(res.headers.get('x-content-type-options'), 'nosniff');
  });

  test('static files are served without path traversal', async () => {
    const c = client(srv.base);
    assert.equal((await c.get('/css/styles.css')).status, 200);
    assert.equal((await c.get('/%2e%2e/package.json')).status, 404);
    assert.equal((await c.get('/../server/app.js')).status, 404);
  });
});

describe('brute-force protection', () => {
  let srv;
  before(async () => {
    srv = await startServer();
    await client(srv.base).post('/api/setup', { token: 'test-setup-token', username: 'owner', password: 'a strong passphrase' });
  });
  after(() => srv.close());

  test('locks an address out after 5 failed logins', async () => {
    const c = client(srv.base);
    for (let i = 0; i < 5; i++) assert.equal((await c.post('/api/login', { username: 'owner', password: `wrong-${i}` })).status, 401);
    const blocked = await c.post('/api/login', { username: 'owner', password: 'a strong passphrase' });
    assert.equal(blocked.status, 429);
    assert.ok(Number(blocked.headers.get('retry-after')) > 0);
  });
});

describe('two-factor authentication', () => {
  let srv;
  let c;
  before(async () => {
    srv = await startServer();
    c = client(srv.base);
    await c.post('/api/setup', { token: 'test-setup-token', username: 'owner', password: 'a strong passphrase' });
  });
  after(() => srv.close());

  test('enable, then login needs a code', async () => {
    const { data } = await c.post('/api/2fa/setup');
    assert.match(data.uri, /^otpauth:\/\/totp\//);
    assert.equal((await c.post('/api/2fa/enable', { code: '000000' })).status, 400);
    const counter = Math.floor(Date.now() / 30000);
    assert.equal((await c.post('/api/2fa/enable', { code: totpCode(data.secret, counter) })).status, 200);
    assert.equal((await c.get('/api/me')).data.totpEnabled, true);

    const other = client(srv.base);
    const needCode = await other.post('/api/login', { username: 'owner', password: 'a strong passphrase' });
    assert.equal(needCode.status, 401);
    assert.equal(needCode.data.totpRequired, true);
    const reused = await other.post('/api/login', {
      username: 'owner',
      password: 'a strong passphrase',
      code: totpCode(data.secret, counter),
    });
    assert.equal(reused.status, 401, 'a code cannot be used twice');
    const ok = await other.post('/api/login', {
      username: 'owner',
      password: 'a strong passphrase',
      code: totpCode(data.secret, counter + 1),
    });
    assert.equal(ok.status, 200);
  });
});

describe('habits, check-ins and notes', () => {
  let srv;
  let c;
  before(async () => {
    srv = await startServer();
    c = client(srv.base);
    await c.post('/api/setup', { token: 'test-setup-token', username: 'owner', password: 'a strong passphrase' });
  });
  after(() => srv.close());

  test('create habits with validation and default colors', async () => {
    assert.equal((await c.post('/api/habits', { name: '', goal: 5 })).status, 400);
    assert.equal((await c.post('/api/habits', { name: 'Run', goal: 40 })).status, 400);
    assert.equal((await c.post('/api/habits', { name: 'Run', goal: 5, color: 'neon' })).status, 400);
    const a = await c.post('/api/habits', { name: '  Journal  ', goal: 20 });
    const b = await c.post('/api/habits', { name: 'Exercise', goal: 15 });
    assert.equal(a.data.name, 'Journal');
    assert.equal(a.data.color, 'amber');
    assert.equal(b.data.color, 'green');
  });

  test('check-ins toggle and come back per month', async () => {
    const { data } = await c.get('/api/month?month=2026-10');
    const [journal] = data.habits;
    for (const date of ['2026-10-01', '2026-10-02', '2026-10-02', '2026-11-01']) {
      assert.equal((await c.put('/api/checks', { habitId: journal.id, date, done: true })).status, 200);
    }
    assert.equal((await c.put('/api/checks', { habitId: journal.id, date: '2026-02-30', done: true })).status, 400);
    assert.equal((await c.put('/api/checks', { habitId: 9999, date: '2026-10-03', done: true })).status, 404);
    let month = (await c.get('/api/month?month=2026-10')).data;
    assert.deepEqual(month.checks[journal.id], ['2026-10-01', '2026-10-02']);
    await c.put('/api/checks', { habitId: journal.id, date: '2026-10-01', done: false });
    month = (await c.get('/api/month?month=2026-10')).data;
    assert.deepEqual(month.checks[journal.id], ['2026-10-02']);
    assert.equal((await c.get('/api/month?month=2026-13')).status, 400);
  });

  test('reorder, edit and archive habits', async () => {
    let { habits } = (await c.get('/api/month?month=2026-10')).data;
    const ids = habits.map((x) => x.id).reverse();
    await c.post('/api/habits/reorder', { ids });
    habits = (await c.get('/api/month?month=2026-10')).data.habits;
    assert.deepEqual(
      habits.map((x) => x.id),
      ids,
    );
    const exercise = habits.find((x) => x.name === 'Exercise');
    const journal = habits.find((x) => x.name === 'Journal');
    const edited = await c.patch(`/api/habits/${exercise.id}`, { name: 'Workout', goal: 12, color: 'teal' });
    assert.equal(edited.data.name, 'Workout');
    assert.equal(edited.data.color, 'teal');

    // Archived habits disappear from months without history, but stay where they were used.
    await c.patch(`/api/habits/${journal.id}`, { archived: true });
    await c.patch(`/api/habits/${exercise.id}`, { archived: true });
    const oct = (await c.get('/api/month?month=2026-10')).data.habits;
    assert.deepEqual(
      oct.map((x) => [x.name, x.archived]),
      [['Journal', true]],
    );
    assert.equal((await c.get('/api/month?month=2026-12')).data.habits.length, 0);
    const archived = (await c.get('/api/habits/archived')).data;
    assert.equal(archived.length, 2);
    assert.equal(archived.find((x) => x.name === 'Journal').totalChecks, 2);

    await c.patch(`/api/habits/${journal.id}`, { archived: false });
    assert.equal((await c.get('/api/month?month=2026-12')).data.habits[0].name, 'Journal');
    assert.equal((await c.del(`/api/habits/${exercise.id}`)).status, 200);
    assert.equal((await c.del(`/api/habits/${exercise.id}`)).status, 404);
  });

  test('notes CRUD', async () => {
    assert.equal((await c.post('/api/notes', { date: '2026-10-05', body: '   ' })).status, 400);
    const note = (await c.post('/api/notes', { date: '2026-10-05', body: 'Felt strong today.' })).data;
    await c.post('/api/notes', { date: '2026-09-30', body: 'Last month' });
    let notes = (await c.get('/api/month?month=2026-10')).data.notes;
    assert.deepEqual(
      notes.map((n) => n.body),
      ['Felt strong today.'],
    );
    await c.patch(`/api/notes/${note.id}`, { body: 'Felt unstoppable today.' });
    notes = (await c.get('/api/month?month=2026-10')).data.notes;
    assert.equal(notes[0].body, 'Felt unstoppable today.');
    assert.equal((await c.del(`/api/notes/${note.id}`)).status, 200);
    assert.equal((await c.get('/api/month?month=2026-10')).data.notes.length, 0);
  });

  test('export and import round-trip', async () => {
    const exported = await c.get('/api/export');
    assert.match(exported.headers.get('content-disposition'), /attachment; filename="cadence-backup-/);
    const backup = exported.data;
    assert.equal(backup.app, 'cadence');
    await c.post('/api/habits', { name: 'Temporary', goal: 3 });
    assert.equal((await c.post('/api/import', { nope: true })).status, 400);
    const res = await c.post('/api/import', backup);
    assert.equal(res.status, 200);
    const again = (await c.get('/api/export')).data;
    assert.deepEqual(
      again.habits.map((x) => [x.name, x.checks]),
      backup.habits.map((x) => [x.name, x.checks]),
    );
    assert.deepEqual(
      again.notes.map((n) => n.body),
      backup.notes.map((n) => n.body),
    );
  });

  test('password change signs out other sessions', async () => {
    const other = client(srv.base);
    await other.post('/api/login', { username: 'owner', password: 'a strong passphrase' });
    assert.equal((await other.get('/api/me')).status, 200);
    assert.equal((await c.post('/api/password', { current: 'wrong password', next: 'brand new pass' })).status, 403);
    const res = await c.post('/api/password', { current: 'a strong passphrase', next: 'brand new pass' });
    assert.equal(res.status, 200);
    assert.equal(res.data.signedOut, 1);
    assert.equal((await other.get('/api/me')).status, 401);
    assert.equal((await c.get('/api/me')).status, 200, 'the current session survives');
  });
});
