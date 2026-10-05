// Small DOM + API helpers shared by every page. No framework, no build step.

const SVG_NS = 'http://www.w3.org/2000/svg';

const ICONS = {
  chevronLeft: '<path d="M15 18l-6-6 6-6"/>',
  chevronRight: '<path d="M9 18l6-6-6-6"/>',
  chevronDown: '<path d="M6 9l6 6 6-6"/>',
  check: '<path d="M4.5 12.5l4.5 4.5 10.5-11"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  x: '<path d="M18 6L6 18M6 6l12 12"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
  moon: '<path d="M21 12.8A9 9 0 1111.2 3a7 7 0 009.8 9.8z"/>',
  archive: '<rect x="3" y="4" width="18" height="4" rx="1"/><path d="M5 8v11a1 1 0 001 1h12a1 1 0 001-1V8M10 12h4"/>',
  settings: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 00.3 1.8l.1.1a2 2 0 11-2.8 2.8l-.1-.1a1.7 1.7 0 00-1.8-.3 1.7 1.7 0 00-1 1.5V21a2 2 0 11-4 0v-.1a1.7 1.7 0 00-1.1-1.5 1.7 1.7 0 00-1.8.3l-.1.1a2 2 0 11-2.8-2.8l.1-.1a1.7 1.7 0 00.3-1.8 1.7 1.7 0 00-1.5-1H3a2 2 0 110-4h.1a1.7 1.7 0 001.5-1.1 1.7 1.7 0 00-.3-1.8l-.1-.1a2 2 0 112.8-2.8l.1.1a1.7 1.7 0 001.8.3H9a1.7 1.7 0 001-1.5V3a2 2 0 114 0v.1a1.7 1.7 0 001 1.5 1.7 1.7 0 001.8-.3l.1-.1a2 2 0 112.8 2.8l-.1.1a1.7 1.7 0 00-.3 1.8V9a1.7 1.7 0 001.5 1H21a2 2 0 110 4h-.1a1.7 1.7 0 00-1.5 1z"/>',
  download: '<path d="M12 3v12M7 10l5 5 5-5M5 21h14"/>',
  upload: '<path d="M12 21V9M7 14l5-5 5 5M5 3h14"/>',
  logout: '<path d="M9 21H5a2 2 0 01-2-2V5a2 2 0 012-2h4M16 17l5-5-5-5M21 12H9"/>',
  pencil: '<path d="M12 20h9M16.5 3.5a2.1 2.1 0 113 3L7 19l-4 1 1-4z"/>',
  trash: '<path d="M3 6h18M8 6V4a1 1 0 011-1h6a1 1 0 011 1v2M19 6l-1 14a1 1 0 01-1 1H7a1 1 0 01-1-1L5 6"/>',
  arrowUp: '<path d="M12 19V5M5 12l7-7 7 7"/>',
  arrowDown: '<path d="M12 5v14M19 12l-7 7-7-7"/>',
  copy: '<rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15H4a1 1 0 01-1-1V4a1 1 0 011-1h10a1 1 0 011 1v1"/>',
  shield: '<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>',
};

export function icon(name, className = 'icon') {
  const svg = document.createElementNS(SVG_NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('class', className);
  svg.setAttribute('aria-hidden', 'true');
  svg.innerHTML = ICONS[name] ?? '';
  return svg;
}

/** h('button', { class: 'btn', onclick: fn, text: 'Save' }, ...children) */
export function h(tag, props = {}, ...children) {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(props ?? {})) {
    if (value === undefined || value === null || value === false) continue;
    if (key === 'class') el.className = value;
    else if (key === 'text') el.textContent = value;
    else if (key === 'value') el.value = value;
    else if (key === 'dataset') Object.assign(el.dataset, value);
    else if (key === 'style') for (const [prop, v] of Object.entries(value)) el.style.setProperty(prop, v);
    else if (key.startsWith('on') && typeof value === 'function') el.addEventListener(key.slice(2), value);
    else if (key in el && typeof value !== 'string') el[key] = value;
    else el.setAttribute(key, value === true ? '' : value);
  }
  append(el, children);
  return el;
}

function append(el, children) {
  for (const child of children.flat(Infinity)) {
    if (child === null || child === undefined || child === false) continue;
    el.append(child instanceof Node ? child : document.createTextNode(String(child)));
  }
}

export class ApiError extends Error {
  constructor(message, status, data) {
    super(message);
    this.status = status;
    this.data = data;
  }
}

export async function api(path, { method = 'GET', body, redirectOn401 = true } = {}) {
  const headers = { 'X-Requested-With': 'fetch' };
  if (body !== undefined) headers['Content-Type'] = 'application/json';
  let res;
  try {
    res = await fetch(`/api${path}`, {
      method,
      headers,
      credentials: 'same-origin',
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch {
    throw new ApiError('Couldn’t reach the server. Check your connection and try again.', 0, {});
  }
  const isJson = (res.headers.get('content-type') || '').includes('application/json');
  const data = isJson ? await res.json().catch(() => ({})) : {};
  if (res.status === 401 && redirectOn401) {
    location.assign('/login');
    throw new ApiError('Please sign in again.', 401, data);
  }
  if (!res.ok) throw new ApiError(data.error || `Request failed (${res.status}).`, res.status, data);
  return data;
}

// ---- toasts -------------------------------------------------------------------
export function toast(message, { type = 'info', duration = 3200 } = {}) {
  let region = document.getElementById('toasts');
  if (!region) {
    region = h('div', { id: 'toasts', class: 'toasts', 'aria-live': 'polite' });
    document.body.append(region);
  }
  const el = h('div', { class: `toast${type === 'error' ? ' error' : ''}`, role: type === 'error' ? 'alert' : 'status', text: message });
  region.append(el);
  setTimeout(() => {
    el.classList.add('leaving');
    setTimeout(() => el.remove(), 260);
  }, duration);
}

// ---- modals -------------------------------------------------------------------
let modalSeq = 0;

export function openModal({ title, wide = false, content, onClose }) {
  const titleId = `modal-title-${++modalSeq}`;
  const dialog = h('dialog', { class: `modal${wide ? ' wide' : ''}`, 'aria-labelledby': titleId });
  const close = () => dialog.open && dialog.close();
  const body = h('div', { class: 'modal-body' });
  dialog.append(
    h(
      'div',
      { class: 'modal-head' },
      h('h2', { id: titleId, text: title }),
      h('button', { class: 'icon-btn', type: 'button', 'aria-label': 'Close', onclick: close }, icon('x')),
    ),
    body,
  );
  // Close when a click both starts and ends on the backdrop.
  let downOnBackdrop = false;
  dialog.addEventListener('mousedown', (e) => (downOnBackdrop = e.target === dialog));
  dialog.addEventListener('click', (e) => {
    if (downOnBackdrop && e.target === dialog) close();
  });
  dialog.addEventListener('close', () => {
    dialog.remove();
    onClose?.();
  });
  document.body.append(dialog);
  content(body, close, dialog);
  dialog.showModal();
  return { dialog, close };
}

export function field(labelText, control, { required = false, hint } = {}) {
  const id = control.id || `f-${Math.random().toString(36).slice(2, 9)}`;
  control.id = id;
  return h(
    'div',
    { class: 'field' },
    h('label', { for: id, class: required ? 'req' : undefined, text: labelText }),
    control,
    hint ? h('p', { class: 'hint', text: hint }) : null,
  );
}

/** First click arms the button, second click within 4s runs the action. */
export function armedButton({ label, armedLabel, className = 'btn btn-danger', onConfirm, iconName }) {
  let timer;
  const btn = h('button', { class: className, type: 'button' });
  const setLabel = (text) => btn.replaceChildren(...(iconName ? [icon(iconName)] : []), text);
  setLabel(label);
  btn.addEventListener('click', async () => {
    if (!btn.classList.contains('is-armed')) {
      btn.classList.add('is-armed');
      setLabel(armedLabel);
      timer = setTimeout(() => {
        btn.classList.remove('is-armed');
        setLabel(label);
      }, 4000);
      return;
    }
    clearTimeout(timer);
    btn.disabled = true;
    try {
      await onConfirm();
    } finally {
      btn.disabled = false;
      btn.classList.remove('is-armed');
      setLabel(label);
    }
  });
  return btn;
}

export function showError(el, message) {
  el.textContent = message;
  el.hidden = !message;
}

// ---- theme ----------------------------------------------------------------------
const THEME_KEY = 'cadence-theme';

export function getThemePref() {
  try {
    const value = localStorage.getItem(THEME_KEY);
    return value === 'light' || value === 'dark' ? value : 'system';
  } catch {
    return 'system';
  }
}

export function setThemePref(pref) {
  try {
    if (pref === 'system') localStorage.removeItem(THEME_KEY);
    else localStorage.setItem(THEME_KEY, pref);
  } catch {
    // storage unavailable — the choice still applies to this page view
  }
  if (pref === 'system') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = pref;
}

export function effectiveTheme() {
  const pref = getThemePref();
  if (pref !== 'system') return pref;
  return matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}
