import fs from 'node:fs';
import path from 'node:path';
import { DatabaseSync } from 'node:sqlite';

export const COLORS = ['amber', 'green', 'blue', 'violet', 'rose', 'orange', 'teal', 'slate'];

const MIGRATIONS = [
  `
  CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    username TEXT NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT NOT NULL,
    totp_secret TEXT,
    totp_pending TEXT,
    totp_last_counter INTEGER NOT NULL DEFAULT -1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
  );

  CREATE TABLE sessions (
    id_hash TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    last_seen INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    user_agent TEXT,
    ip TEXT
  );
  CREATE INDEX sessions_user ON sessions(user_id);

  CREATE TABLE habits (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    goal INTEGER NOT NULL,
    color TEXT NOT NULL,
    position INTEGER NOT NULL,
    archived_at TEXT,
    created_at TEXT NOT NULL
  );

  CREATE TABLE checks (
    habit_id INTEGER NOT NULL REFERENCES habits(id) ON DELETE CASCADE,
    date TEXT NOT NULL,
    PRIMARY KEY (habit_id, date)
  ) WITHOUT ROWID;
  CREATE INDEX checks_date ON checks(date);

  CREATE TABLE notes (
    id INTEGER PRIMARY KEY,
    date TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
  );
  CREATE INDEX notes_date ON notes(date);
  `,
];

const nowIso = () => new Date().toISOString();

function migrate(db) {
  const { user_version: version } = db.prepare('PRAGMA user_version').get();
  for (let i = version; i < MIGRATIONS.length; i++) {
    db.exec('BEGIN');
    try {
      db.exec(MIGRATIONS[i]);
      db.exec(`PRAGMA user_version = ${i + 1}`);
      db.exec('COMMIT');
    } catch (err) {
      db.exec('ROLLBACK');
      throw err;
    }
  }
}

export function openDb(file) {
  if (file !== ':memory:') fs.mkdirSync(path.dirname(path.resolve(file)), { recursive: true });
  const db = new DatabaseSync(file);
  db.exec('PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;');
  migrate(db);
  return createStore(db);
}

function createStore(db) {
  const cache = new Map();
  const q = (sql) => {
    let stmt = cache.get(sql);
    if (!stmt) cache.set(sql, (stmt = db.prepare(sql)));
    return stmt;
  };
  const tx = (fn) => {
    db.exec('BEGIN IMMEDIATE');
    try {
      const result = fn();
      db.exec('COMMIT');
      return result;
    } catch (err) {
      db.exec('ROLLBACK');
      throw err;
    }
  };

  const store = {
    close: () => db.close(),

    // ---- users -------------------------------------------------------------
    countUsers: () => q('SELECT COUNT(*) AS n FROM users').get().n,
    getUserById: (id) => q('SELECT * FROM users WHERE id = ?').get(id),
    getUserByUsername: (username) => q('SELECT * FROM users WHERE username = ?').get(username),
    getOwner: () => q('SELECT * FROM users ORDER BY id LIMIT 1').get(),
    createUser(username, passwordHash) {
      const ts = nowIso();
      const { lastInsertRowid } = q(
        'INSERT INTO users (username, password_hash, created_at, updated_at) VALUES (?, ?, ?, ?)',
      ).run(username, passwordHash, ts, ts);
      return store.getUserById(Number(lastInsertRowid));
    },
    updateCredentials(id, username, passwordHash) {
      q('UPDATE users SET username = ?, password_hash = ?, updated_at = ? WHERE id = ?').run(
        username,
        passwordHash,
        nowIso(),
        id,
      );
    },
    updatePassword(id, passwordHash) {
      q('UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?').run(passwordHash, nowIso(), id);
    },
    setTotpPending: (id, secret) => q('UPDATE users SET totp_pending = ? WHERE id = ?').run(secret, id),
    enableTotp(id, secret, counter) {
      q(
        'UPDATE users SET totp_secret = ?, totp_pending = NULL, totp_last_counter = ?, updated_at = ? WHERE id = ?',
      ).run(secret, counter, nowIso(), id);
    },
    disableTotp(id) {
      q(
        'UPDATE users SET totp_secret = NULL, totp_pending = NULL, totp_last_counter = -1, updated_at = ? WHERE id = ?',
      ).run(nowIso(), id);
    },
    setTotpCounter: (id, counter) => q('UPDATE users SET totp_last_counter = ? WHERE id = ?').run(counter, id),

    // ---- sessions ----------------------------------------------------------
    createSession({ idHash, userId, expiresAt, userAgent, ip }) {
      const now = Date.now();
      q(
        'INSERT INTO sessions (id_hash, user_id, created_at, last_seen, expires_at, user_agent, ip) VALUES (?, ?, ?, ?, ?, ?, ?)',
      ).run(idHash, userId, now, now, expiresAt, userAgent, ip);
    },
    getSession: (idHash) => q('SELECT * FROM sessions WHERE id_hash = ?').get(idHash),
    touchSession: (idHash, lastSeen, expiresAt) =>
      q('UPDATE sessions SET last_seen = ?, expires_at = ? WHERE id_hash = ?').run(lastSeen, expiresAt, idHash),
    deleteSession: (idHash) => q('DELETE FROM sessions WHERE id_hash = ?').run(idHash),
    deleteOtherSessions: (userId, keepIdHash) =>
      q('DELETE FROM sessions WHERE user_id = ? AND id_hash != ?').run(userId, keepIdHash ?? '').changes,
    listSessions: (userId) =>
      q(
        'SELECT id_hash, created_at, last_seen, user_agent, ip FROM sessions WHERE user_id = ? AND expires_at > ? ORDER BY last_seen DESC',
      ).all(userId, Date.now()),
    purgeExpiredSessions: () => q('DELETE FROM sessions WHERE expires_at <= ?').run(Date.now()),

    // ---- habits ------------------------------------------------------------
    getHabit: (id) => q('SELECT * FROM habits WHERE id = ?').get(id),
    countActiveHabits: () => q('SELECT COUNT(*) AS n FROM habits WHERE archived_at IS NULL').get().n,

    /** Active habits plus archived ones that still have history in this month. */
    getMonth(start, end) {
      const habits = q(
        `SELECT * FROM habits h
         WHERE h.archived_at IS NULL
            OR EXISTS (SELECT 1 FROM checks c WHERE c.habit_id = h.id AND c.date BETWEEN ? AND ?)
         ORDER BY h.archived_at IS NOT NULL, h.position, h.id`,
      ).all(start, end);
      const checks = q('SELECT habit_id, date FROM checks WHERE date BETWEEN ? AND ? ORDER BY date').all(start, end);
      const notes = q('SELECT * FROM notes WHERE date BETWEEN ? AND ? ORDER BY date DESC, id DESC').all(start, end);
      return { habits, checks, notes };
    },

    listArchived: () =>
      q(
        `SELECT h.*, (SELECT COUNT(*) FROM checks c WHERE c.habit_id = h.id) AS total_checks
         FROM habits h WHERE h.archived_at IS NOT NULL ORDER BY h.archived_at DESC`,
      ).all(),

    createHabit({ name, goal, color }) {
      return tx(() => {
        const { p } = q('SELECT COALESCE(MAX(position), -1) + 1 AS p FROM habits').get();
        const { lastInsertRowid } = q(
          'INSERT INTO habits (name, goal, color, position, created_at) VALUES (?, ?, ?, ?, ?)',
        ).run(name, goal, color, p, nowIso());
        return store.getHabit(Number(lastInsertRowid));
      });
    },

    updateHabit(id, { name, goal, color, archived }) {
      const habit = store.getHabit(id);
      if (!habit) return null;
      const archivedAt =
        archived === undefined ? habit.archived_at : archived ? (habit.archived_at ?? nowIso()) : null;
      tx(() => {
        q('UPDATE habits SET name = ?, goal = ?, color = ?, archived_at = ? WHERE id = ?').run(
          name ?? habit.name,
          goal ?? habit.goal,
          color ?? habit.color,
          archivedAt,
          id,
        );
        // A restored habit goes back to the bottom of the list.
        if (habit.archived_at && archived === false) {
          const { p } = q('SELECT COALESCE(MAX(position), -1) + 1 AS p FROM habits WHERE archived_at IS NULL AND id != ?').get(id);
          q('UPDATE habits SET position = ? WHERE id = ?').run(p, id);
        }
      });
      return store.getHabit(id);
    },

    deleteHabit: (id) => q('DELETE FROM habits WHERE id = ?').run(id).changes > 0,

    reorderHabits(ids) {
      tx(() => {
        const stmt = q('UPDATE habits SET position = ? WHERE id = ?');
        ids.forEach((id, index) => stmt.run(index, id));
      });
    },

    setCheck(habitId, date, done) {
      if (done) q('INSERT OR IGNORE INTO checks (habit_id, date) VALUES (?, ?)').run(habitId, date);
      else q('DELETE FROM checks WHERE habit_id = ? AND date = ?').run(habitId, date);
    },

    // ---- notes -------------------------------------------------------------
    getNote: (id) => q('SELECT * FROM notes WHERE id = ?').get(id),
    createNote({ date, body }) {
      const ts = nowIso();
      const { lastInsertRowid } = q('INSERT INTO notes (date, body, created_at, updated_at) VALUES (?, ?, ?, ?)').run(
        date,
        body,
        ts,
        ts,
      );
      return store.getNote(Number(lastInsertRowid));
    },
    updateNote(id, { date, body }) {
      const note = store.getNote(id);
      if (!note) return null;
      q('UPDATE notes SET date = ?, body = ?, updated_at = ? WHERE id = ?').run(
        date ?? note.date,
        body ?? note.body,
        nowIso(),
        id,
      );
      return store.getNote(id);
    },
    deleteNote: (id) => q('DELETE FROM notes WHERE id = ?').run(id).changes > 0,

    // ---- backup ------------------------------------------------------------
    exportAll() {
      const checks = q('SELECT habit_id, date FROM checks ORDER BY date').all();
      const byHabit = new Map();
      for (const c of checks) {
        if (!byHabit.has(c.habit_id)) byHabit.set(c.habit_id, []);
        byHabit.get(c.habit_id).push(c.date);
      }
      const habits = q('SELECT * FROM habits ORDER BY position, id')
        .all()
        .map((h) => ({
          name: h.name,
          goal: h.goal,
          color: h.color,
          position: h.position,
          archived_at: h.archived_at,
          created_at: h.created_at,
          checks: byHabit.get(h.id) ?? [],
        }));
      const notes = q('SELECT date, body, created_at, updated_at FROM notes ORDER BY date, id').all();
      return { habits, notes };
    },

    /** Replaces every habit, check and note with the given (pre-validated) backup. */
    importAll({ habits, notes }) {
      tx(() => {
        db.exec('DELETE FROM checks; DELETE FROM habits; DELETE FROM notes;');
        const insHabit = q(
          'INSERT INTO habits (name, goal, color, position, archived_at, created_at) VALUES (?, ?, ?, ?, ?, ?)',
        );
        const insCheck = q('INSERT OR IGNORE INTO checks (habit_id, date) VALUES (?, ?)');
        const insNote = q('INSERT INTO notes (date, body, created_at, updated_at) VALUES (?, ?, ?, ?)');
        habits.forEach((h, i) => {
          const { lastInsertRowid } = insHabit.run(h.name, h.goal, h.color, i, h.archived_at, h.created_at);
          for (const d of h.checks) insCheck.run(lastInsertRowid, d);
        });
        for (const n of notes) insNote.run(n.date, n.body, n.created_at, n.updated_at);
      });
    },
  };

  return store;
}
