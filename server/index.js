import http from 'node:http';
import { createApp } from './app.js';
import { loadConfig } from './config.js';
import { openDb } from './db.js';

const config = loadConfig();
const store = openDb(config.dbPath);
const app = await createApp({ store, config, log: console });

const server = http.createServer(app.handle);
server.headersTimeout = 20_000;
server.requestTimeout = 30_000;

server.listen(config.port, config.host, () => {
  const local = `http://localhost:${config.port}`;
  console.log(`${config.appName} is running at ${local} (database: ${config.dbPath})`);
  if (app.setupToken) {
    console.log('');
    console.log('No account exists yet. Create yours here (this link works once):');
    console.log(`  ${local}/setup?token=${app.setupToken}`);
    console.log(`  Setup token: ${app.setupToken}`);
    console.log('');
  }
});

function shutdown() {
  server.close(() => {
    store.close();
    process.exit(0);
  });
  setTimeout(() => process.exit(0), 5000).unref();
}
process.on('SIGTERM', shutdown);
process.on('SIGINT', shutdown);
