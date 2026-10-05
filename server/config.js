import path from 'node:path';

const bool = (value) => value === '1' || value === 'true' || value === 'yes';

export function loadConfig(env = process.env) {
  return {
    port: Number(env.PORT) || 3000,
    host: env.HOST || '0.0.0.0',
    dbPath: env.DB_PATH || path.join(process.cwd(), 'data', 'cadence.db'),
    // Set when running behind a reverse proxy (Caddy, nginx, Fly, Render...) so the
    // client IP and HTTPS scheme are read from X-Forwarded-* headers.
    trustProxy: bool(env.TRUST_PROXY),
    // undefined = decide per request (Secure when the request arrived over HTTPS).
    cookieSecure: env.COOKIE_SECURE === undefined ? undefined : bool(env.COOKIE_SECURE),
    sessionDays: Number(env.SESSION_DAYS) || 30,
    setupToken: env.SETUP_TOKEN || null,
    appName: env.APP_NAME || 'Cadence',
  };
}
