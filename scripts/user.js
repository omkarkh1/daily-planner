// Account maintenance from the server's shell. Anyone who can run this already
// controls the data, so it is the recovery path for a forgotten password or a
// lost authenticator.
//
//   npm run user -- passwd        create the account, or reset username + password
//   npm run user -- disable-2fa   turn off two-factor authentication
//   npm run user -- signout-all   end every active session
//   npm run user -- status        show account details
import readline from 'node:readline';
import { PASSWORD_MAX, PASSWORD_MIN, hashPassword } from '../server/auth.js';
import { loadConfig } from '../server/config.js';
import { openDb } from '../server/db.js';

const USAGE = 'Usage: npm run user -- <passwd | disable-2fa | signout-all | status>';
const command = process.argv[2];
if (!['passwd', 'disable-2fa', 'signout-all', 'status'].includes(command)) {
  console.log(USAGE);
  process.exit(command ? 1 : 0);
}

const config = loadConfig();
const store = openDb(config.dbPath);

// One reader for the whole run, so piped input (`printf 'me\npass\npass\n' | ...`) works too.
const tty = Boolean(process.stdin.isTTY);
const rl = readline.createInterface({ input: process.stdin, output: process.stdout, terminal: tty });
let muted = false;
rl._writeToOutput = (text) => {
  if (!muted) process.stdout.write(text);
};
const buffered = [];
const waiting = [];
rl.on('line', (line) => (waiting.length ? waiting.shift()(line) : buffered.push(line)));
rl.on('close', () => waiting.splice(0).forEach((resolve) => resolve(null)));

function ask(question, { hidden = false } = {}) {
  process.stdout.write(question);
  muted = hidden && tty;
  return new Promise((resolve, reject) => {
    const done = (answer) => {
      if (muted) process.stdout.write('\n');
      muted = false;
      if (answer === null) reject(new Error('\nInput ended before all questions were answered.'));
      else resolve(answer);
    };
    if (buffered.length) done(buffered.shift());
    else waiting.push(done);
  });
}

async function passwd() {
  const owner = store.getOwner();
  const fallback = owner?.username ?? '';
  const username = (await ask(`Username${fallback ? ` [${fallback}]` : ''}: `)).trim() || fallback;
  if (!username || username.length > 64) throw new Error('Username must be 1–64 characters.');
  const password = await ask('New password: ', { hidden: true });
  if (password.length < PASSWORD_MIN || password.length > PASSWORD_MAX) {
    throw new Error(`Password must be at least ${PASSWORD_MIN} characters.`);
  }
  const confirm = await ask('Confirm password: ', { hidden: true });
  if (password !== confirm) throw new Error('Passwords do not match.');
  const hash = await hashPassword(password);
  if (owner) {
    store.updateCredentials(owner.id, username, hash);
    store.deleteOtherSessions(owner.id, null);
    console.log(`Updated credentials for "${username}". All sessions were signed out.`);
  } else {
    store.createUser(username, hash);
    console.log(`Created account "${username}". Sign in at ${config.appName}'s /login page.`);
  }
}

function requireOwner() {
  const owner = store.getOwner();
  if (!owner) throw new Error('No account exists yet. Run: npm run user -- passwd');
  return owner;
}

const commands = {
  passwd,
  'disable-2fa'() {
    const owner = requireOwner();
    store.disableTotp(owner.id);
    console.log(`Two-factor authentication is off for "${owner.username}".`);
  },
  'signout-all'() {
    const owner = requireOwner();
    const count = store.deleteOtherSessions(owner.id, null);
    console.log(`Signed out ${count} session${count === 1 ? '' : 's'}.`);
  },
  status() {
    const owner = requireOwner();
    console.log(`Username:  ${owner.username}`);
    console.log(`2FA:       ${owner.totp_secret ? 'on' : 'off'}`);
    console.log(`Sessions:  ${store.listSessions(owner.id).length} active`);
    console.log(`Database:  ${config.dbPath}`);
  },
};

try {
  await commands[command]();
} catch (err) {
  console.error(err.message);
  process.exitCode = 1;
} finally {
  rl.close();
  store.close();
}
