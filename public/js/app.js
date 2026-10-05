import {
  api,
  armedButton,
  effectiveTheme,
  field,
  getThemePref,
  h,
  icon,
  openModal,
  setThemePref,
  showError,
  toast,
} from './ui.js';

// ---- constants ----------------------------------------------------------------
const COLORS = ['amber', 'green', 'blue', 'violet', 'rose', 'orange', 'teal', 'slate'];
const COLOR_NAMES = {
  amber: 'Butter',
  green: 'Sage',
  blue: 'Sky',
  violet: 'Lavender',
  rose: 'Blush',
  orange: 'Apricot',
  teal: 'Mint',
  slate: 'Stone',
};
const SUGGESTIONS = [
  ['Exercise', 20],
  ['Read 20 pages', 25],
  ['Journal', 20],
  ['Meditate 10 min', 25],
  ['Sleep by 11 pm', 25],
  ['Drink 2L of water', 28],
  ['No added sugar', 20],
  ['Learn something new', 15],
];
const GOAL_MESSAGES = [
  (h) => `Goal reached: ${h.name}. ${h.goal} times this month — that’s who you are now.`,
  (h) => `${h.name}: ${h.goal}/${h.goal}. You said you would, and you did.`,
  (h) => `Monthly goal crushed for ${h.name}. Keep raising the bar.`,
];
const PERFECT_DAY = [
  'Every habit done today. This is what showing up looks like.',
  'Clean sweep today. Future you is grateful.',
  'All habits checked. One more day of proof.',
];

// ---- date helpers -------------------------------------------------------------
const pad = (n) => String(n).padStart(2, '0');
const ymd = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const monthOf = (date) => date.slice(0, 7);
const splitMonth = (key) => key.split('-').map(Number);
const shiftMonth = (key, delta) => {
  const [y, m] = splitMonth(key);
  const d = new Date(y, m - 1 + delta, 1);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}`;
};
const daysIn = (key) => {
  const [y, m] = splitMonth(key);
  return new Date(y, m, 0).getDate();
};
const dateOf = (key, day) => `${key}-${pad(day)}`;
const asDate = (date) => {
  const [y, m, d] = date.split('-').map(Number);
  return new Date(y, m - 1, d);
};
const monthLabel = (key) => {
  const [y, m] = splitMonth(key);
  return `${new Date(y, m - 1, 1).toLocaleString('en-US', { month: 'long' })}, ${y}`;
};
const weekdayLetter = (date) => asDate(date).toLocaleDateString('en-US', { weekday: 'narrow' });
const fullDate = (date) =>
  asDate(date).toLocaleDateString('en-US', { weekday: 'long', month: 'long', day: 'numeric' });
const noteDate = (date) => asDate(date).toLocaleDateString('en-GB', { day: 'numeric', month: 'long', year: 'numeric' });
const isMonthKey = (value) => /^\d{4}-(0[1-9]|1[0-2])$/.test(value);
const pick = (list) => list[Math.floor(Math.random() * list.length)];

// ---- state --------------------------------------------------------------------
const state = {
  month: null,
  today: ymd(new Date()),
  habits: [],
  checks: new Map(),
  notes: [],
  me: null,
  loadSeq: 0,
};

const $ = (id) => document.getElementById(id);
const els = {
  monthLabel: $('month-label'),
  prev: $('prev-month'),
  next: $('next-month'),
  todayBtn: $('today-btn'),
  summary: $('summary'),
  tracker: $('tracker'),
  notesList: $('notes-list'),
  newNote: $('new-note'),
  userBtn: $('user-btn'),
  userMenu: $('user-menu'),
  username: $('username'),
  avatar: $('avatar'),
  themeBtn: $('theme-btn'),
  importFile: $('import-file'),
};

const currentMonth = () => monthOf(state.today);
const activeHabits = () => state.habits.filter((x) => !x.archived);
/** Archived habits stay visible in past months where they have history. */
const visibleHabits = () => state.habits.filter((x) => !x.archived || state.month < currentMonth());
const countFor = (habit) => state.checks.get(habit.id)?.size ?? 0;

// ---- loading ------------------------------------------------------------------
async function load(month, { scrollToToday = true } = {}) {
  const seq = ++state.loadSeq;
  state.month = month;
  if (location.hash.slice(1) !== month) history.replaceState(null, '', `#${month}`);
  renderMonthBar();
  try {
    const data = await api(`/month?month=${month}`);
    if (seq !== state.loadSeq) return;
    state.habits = data.habits;
    state.checks = new Map(data.habits.map((x) => [x.id, new Set(data.checks[x.id] ?? [])]));
    state.notes = data.notes;
    render({ scrollToToday });
  } catch (err) {
    if (seq === state.loadSeq) toast(err.message, { type: 'error' });
  }
}

const refresh = () => load(state.month, { scrollToToday: false });

// ---- rendering ----------------------------------------------------------------
function render({ scrollToToday = false } = {}) {
  renderMonthBar();
  renderSummary();
  renderGrid({ scrollToToday });
  renderNotes();
}

function renderMonthBar() {
  els.monthLabel.textContent = monthLabel(state.month);
  els.todayBtn.hidden = state.month === currentMonth();
  document.title = `${monthLabel(state.month)} · Cadence`;
}

function renderSummary() {
  const habits = visibleHabits().filter((x) => !x.archived);
  const n = habits.length;
  if (!n) return els.summary.replaceChildren();
  const met = habits.filter((x) => countFor(x) >= x.goal).length;
  const total = habits.reduce((sum, x) => sum + countFor(x), 0);
  const parts = [];
  if (state.month === currentMonth()) {
    const doneToday = habits.filter((x) => state.checks.get(x.id)?.has(state.today)).length;
    parts.push([h('strong', { text: `${doneToday}/${n}` }), ' done today']);
  } else if (state.month > currentMonth()) {
    return els.summary.replaceChildren('A fresh month. Every empty box is a chance.');
  }
  parts.push([h('strong', { text: `${met}/${n}` }), ' monthly goals met']);
  parts.push([h('strong', { text: total }), ` check-in${total === 1 ? '' : 's'}`]);
  els.summary.replaceChildren(...parts.flatMap((p, i) => (i ? [' · ', ...p] : p)));
}

function checkIcon() {
  return icon('check', '');
}

function renderGrid({ scrollToToday }) {
  const oldWrap = els.tracker.querySelector('.grid-wrap');
  const previousScroll = oldWrap?.scrollLeft ?? 0;
  const habits = visibleHabits();

  if (!habits.length) {
    els.tracker.replaceChildren(emptyState());
    return;
  }

  const days = daysIn(state.month);
  els.tracker.style.setProperty('--days', days);
  const noteDays = new Set(state.notes.map((n) => n.date));

  const cols = [h('col', { class: 'c-name' })];
  const head = [h('th', { class: 'h-label h-habits', scope: 'col', text: 'Habits' })];
  for (let d = 1; d <= days; d++) {
    const date = dateOf(state.month, d);
    cols.push(h('col', { class: 'c-day' }));
    head.push(
      h(
        'th',
        { class: `day-head${date === state.today ? ' is-today' : ''}`, scope: 'col', title: fullDate(date) },
        h('span', { class: 'dow', text: weekdayLetter(date) }),
        h('span', { class: 'dom', text: d }),
        noteDays.has(date) ? h('span', { class: 'note-dot', 'aria-label': 'Has a note' }) : null,
      ),
    );
  }
  cols.push(h('col', { class: 'c-goal' }), h('col', { class: 'c-ach' }));
  head.push(
    h('th', { class: 'h-label h-goal', scope: 'col', text: 'Goal' }),
    h('th', { class: 'h-label h-ach', scope: 'col', text: 'Achieved' }),
  );

  const table = h(
    'table',
    { class: 'grid' },
    h('caption', { class: 'sr-only', text: `Habits for ${monthLabel(state.month)}` }),
    h('colgroup', {}, cols),
    h('thead', {}, h('tr', {}, head)),
    h('tbody', {}, habits.map((habit) => habitRow(habit, days))),
  );
  const wrap = h('div', { class: 'grid-wrap' }, table);
  els.tracker.replaceChildren(wrap, trackerFoot());

  if (scrollToToday) {
    const todayHead = table.querySelector('.day-head.is-today');
    const nameWidth = table.querySelector('.h-habits').offsetWidth;
    wrap.scrollLeft = todayHead ? Math.max(0, todayHead.offsetLeft - nameWidth - todayHead.offsetWidth * 3) : 0;
  } else {
    wrap.scrollLeft = previousScroll;
  }
}

function habitRow(habit, days) {
  const set = state.checks.get(habit.id) ?? new Set();
  const count = set.size;
  const readOnly = habit.archived;
  const row = h('tr', { class: `hc-${habit.color}${readOnly ? ' is-archived' : ''}`, dataset: { id: habit.id } });

  row.append(
    h(
      'th',
      { class: 'name', scope: 'row' },
      h(
        'button',
        {
          class: 'name-btn',
          type: 'button',
          title: readOnly ? 'Archived — restore it from the menu' : 'Edit habit',
          onclick: () => (readOnly ? openArchived() : openHabitModal(habit)),
        },
        h(
          'span',
          { class: 'name-text' },
          h('span', { class: 'swatch' }),
          h('span', { text: habit.name }),
          readOnly ? h('span', { class: 'tag', text: 'Archived' }) : null,
        ),
        h('span', { class: `mini-progress${count >= habit.goal ? ' met' : ''}`, text: `${count} / ${habit.goal}` }),
      ),
    ),
  );

  for (let d = 1; d <= days; d++) {
    const date = dateOf(state.month, d);
    const cell = h(
      'button',
      {
        class: 'cell',
        type: 'button',
        'aria-pressed': String(set.has(date)),
        'aria-label': `${habit.name}, ${fullDate(date)}`,
        disabled: readOnly || date > state.today,
        dataset: { date },
      },
      checkIcon(),
    );
    cell.addEventListener('click', () => toggle(habit, date, cell));
    row.append(h('td', { class: `day${date === state.today ? ' is-today' : ''}` }, cell));
  }

  row.append(
    h('td', { class: 'goal', text: habit.goal }),
    h('td', { class: `ach ${count >= habit.goal ? 'met' : 'below'}`, text: count }),
  );
  return row;
}

function updateRowProgress(row, habit) {
  const count = countFor(habit);
  const met = count >= habit.goal;
  const ach = row.querySelector('td.ach');
  ach.textContent = count;
  ach.className = `ach ${met ? 'met' : 'below'}`;
  const mini = row.querySelector('.mini-progress');
  mini.textContent = `${count} / ${habit.goal}`;
  mini.classList.toggle('met', met);
}

function trackerFoot() {
  return h(
    'div',
    { class: 'tracker-foot' },
    h('button', { class: 'btn-text', type: 'button', onclick: () => openHabitModal() }, icon('plus'), 'New habit'),
    h(
      'div',
      { class: 'legend', 'aria-hidden': 'true' },
      h('span', {}, h('i', { class: 'l-below' }), 'Building up'),
      h('span', {}, h('i', { class: 'l-met' }), 'Goal met'),
    ),
  );
}

function emptyState() {
  const firstTime = state.habits.length === 0 && state.month >= currentMonth();
  return h(
    'div',
    { class: 'empty' },
    h('h2', { text: firstTime ? 'Let’s build your first habit' : `Nothing tracked in ${monthLabel(state.month)}` }),
    h('p', {
      text: firstTime
        ? 'Set a monthly goal instead of chasing a perfect streak. Missing a day is fine — showing up again is what counts.'
        : 'Every month is a new page. Add a habit and start filling it in.',
    }),
    firstTime
      ? h(
          'div',
          { class: 'chips' },
          SUGGESTIONS.map(([name, goal]) =>
            h('button', { class: 'chip', type: 'button', text: name, onclick: () => openHabitModal(null, { name, goal }) }),
          ),
        )
      : null,
    h('button', { class: 'btn btn-primary', type: 'button', onclick: () => openHabitModal() }, icon('plus'), 'New habit'),
  );
}

function renderNotes() {
  if (!state.notes.length) {
    els.notesList.replaceChildren(
      h('div', {
        class: 'notes-empty',
        text: 'No notes this month yet. Capture a win, a lesson, or a thought worth keeping.',
      }),
    );
    return;
  }
  els.notesList.replaceChildren(
    ...state.notes.map((note) =>
      h(
        'article',
        { class: 'note' },
        h('time', { datetime: note.date, text: noteDate(note.date) }),
        h('p', { text: note.body }),
        h(
          'div',
          { class: 'note-actions' },
          h('button', { class: 'btn btn-sm btn-secondary', type: 'button', onclick: () => openNoteModal(note) }, icon('pencil'), 'Edit'),
          armedButton({
            label: 'Delete',
            armedLabel: 'Confirm',
            className: 'btn btn-sm btn-danger',
            iconName: 'trash',
            onConfirm: async () => {
              try {
                await api(`/notes/${note.id}`, { method: 'DELETE' });
                state.notes = state.notes.filter((n) => n.id !== note.id);
                renderNotes();
                renderGrid({ scrollToToday: false });
              } catch (err) {
                toast(err.message, { type: 'error' });
              }
            },
          }),
        ),
      ),
    ),
  );
}

// ---- checking off -------------------------------------------------------------
const saveQueues = new Map();

function applyCheck(habit, date, done, cell) {
  const set = state.checks.get(habit.id);
  if (done) set.add(date);
  else set.delete(date);
  cell.setAttribute('aria-pressed', String(done));
  updateRowProgress(cell.closest('tr'), habit);
  renderSummary();
}

function toggle(habit, date, cell) {
  const done = !state.checks.get(habit.id).has(date);
  applyCheck(habit, date, done, cell);
  // Saves for the same cell run in order, so rapid taps always settle on the last state.
  const key = `${habit.id}:${date}`;
  const run = (saveQueues.get(key) ?? Promise.resolve())
    .then(() => api('/checks', { method: 'PUT', body: { habitId: habit.id, date, done } }))
    .then(() => {
      if (done && state.checks.get(habit.id)?.has(date)) celebrate(habit, date);
    })
    .catch((err) => {
      toast(err.message, { type: 'error' });
      refresh();
    })
    .finally(() => {
      if (saveQueues.get(key) === run) saveQueues.delete(key);
    });
  saveQueues.set(key, run);
}

function celebrate(habit, date) {
  if (countFor(habit) === habit.goal) {
    toast(pick(GOAL_MESSAGES)(habit), { duration: 4200 });
    return;
  }
  const habits = activeHabits();
  if (date === state.today && habits.length > 1 && habits.every((x) => state.checks.get(x.id)?.has(date))) {
    toast(pick(PERFECT_DAY), { duration: 4200 });
  }
}

// ---- habit modal --------------------------------------------------------------
function openHabitModal(habit = null, preset = {}) {
  openModal({
    title: habit ? 'Edit habit' : 'Create new habit',
    content(body, close) {
      const error = h('p', { class: 'form-error', role: 'alert', hidden: true });
      const name = h('input', {
        class: 'input',
        name: 'name',
        maxlength: '80',
        autocomplete: 'off',
        placeholder: 'e.g. Exercise',
        value: habit?.name ?? preset.name ?? '',
      });
      const goal = h('input', {
        class: 'input',
        name: 'goal',
        type: 'number',
        inputmode: 'numeric',
        min: '1',
        max: '31',
        placeholder: 'Number of times to perform habit in a month',
        value: String(habit?.goal ?? preset.goal ?? ''),
      });
      const selected = habit?.color ?? COLORS[activeHabits().length % 4];
      const swatches = h(
        'div',
        { class: 'swatches', role: 'radiogroup', 'aria-label': 'Color' },
        COLORS.map((c) =>
          h(
            'label',
            { class: `swatch-opt hc-${c}`, title: COLOR_NAMES[c] },
            h('input', { type: 'radio', name: 'color', value: c, checked: c === selected, 'aria-label': COLOR_NAMES[c] }),
            h('span'),
          ),
        ),
      );
      const save = h('button', { class: 'btn btn-primary', type: 'submit', text: 'Save' });
      const form = h(
        'form',
        { novalidate: true },
        error,
        field('Habit name', name, { required: true }),
        field('Goal', goal, {
          required: true,
          hint: 'Flexible consistency beats perfect streaks. How many days this month will you show up?',
        }),
        h('div', { class: 'field' }, h('span', { class: 'label', text: 'Color' }), swatches),
        h('div', { class: 'modal-actions' }, save),
      );

      form.addEventListener('submit', async (event) => {
        event.preventDefault();
        const payload = {
          name: name.value.trim(),
          goal: Number(goal.value),
          color: form.elements.color.value,
        };
        if (!payload.name) return showError(error, 'Give your habit a name.'), name.focus();
        if (!Number.isInteger(payload.goal) || payload.goal < 1 || payload.goal > 31) {
          return showError(error, 'Pick a monthly goal from 1 to 31.'), goal.focus();
        }
        save.disabled = true;
        try {
          if (habit) await api(`/habits/${habit.id}`, { method: 'PATCH', body: payload });
          else await api('/habits', { method: 'POST', body: payload });
          close();
          if (!habit) toast(`“${payload.name}” added. Day one starts now.`);
          await refresh();
        } catch (err) {
          showError(error, err.message);
          save.disabled = false;
        }
      });

      body.append(form);
      if (habit) body.append(...manageHabit(habit, close));
      requestAnimationFrame(() => (habit ? null : name.focus()));
    },
  });
}

function manageHabit(habit, close) {
  const order = activeHabits().map((x) => x.id);
  const index = order.indexOf(habit.id);
  const move = async (delta) => {
    const ids = [...order];
    [ids[index], ids[index + delta]] = [ids[index + delta], ids[index]];
    try {
      await api('/habits/reorder', { method: 'POST', body: { ids } });
      close();
      await refresh();
    } catch (err) {
      toast(err.message, { type: 'error' });
    }
  };

  return [
    h(
      'div',
      { class: 'section' },
      h('h3', { text: 'Order' }),
      h('p', { text: 'Stack your habits in the order you tackle them.' }),
      h(
        'div',
        { class: 'order-row' },
        h('button', { class: 'btn btn-sm btn-secondary', type: 'button', disabled: index <= 0, onclick: () => move(-1) }, icon('arrowUp'), 'Move up'),
        h(
          'button',
          { class: 'btn btn-sm btn-secondary', type: 'button', disabled: index === -1 || index >= order.length - 1, onclick: () => move(1) },
          icon('arrowDown'),
          'Move down',
        ),
      ),
    ),
    h(
      'div',
      { class: 'section' },
      h('h3', { text: 'Wrap it up' }),
      h('p', { text: 'Archive keeps its history in past months. Delete removes the habit and every check-in.' }),
      h(
        'div',
        { class: 'modal-actions' },
        h(
          'button',
          {
            class: 'btn btn-sm btn-secondary',
            type: 'button',
            onclick: async () => {
              try {
                await api(`/habits/${habit.id}`, { method: 'PATCH', body: { archived: true } });
                close();
                toast(`Archived “${habit.name}”. Restore it anytime from the menu.`);
                await refresh();
              } catch (err) {
                toast(err.message, { type: 'error' });
              }
            },
          },
          icon('archive'),
          'Archive',
        ),
        armedButton({
          label: 'Delete',
          armedLabel: 'Click again to delete',
          className: 'btn btn-sm btn-danger',
          iconName: 'trash',
          onConfirm: async () => {
            try {
              await api(`/habits/${habit.id}`, { method: 'DELETE' });
              close();
              await refresh();
            } catch (err) {
              toast(err.message, { type: 'error' });
            }
          },
        }),
      ),
    ),
  ];
}

// ---- notes modal --------------------------------------------------------------
function defaultNoteDate() {
  if (state.month === currentMonth()) return state.today;
  if (state.month < currentMonth()) return dateOf(state.month, daysIn(state.month));
  return dateOf(state.month, 1);
}

function openNoteModal(note = null) {
  openModal({
    title: note ? 'Edit note' : 'Create new note',
    content(body, close) {
      const error = h('p', { class: 'form-error', role: 'alert', hidden: true });
      const date = h('input', { class: 'input', type: 'date', name: 'date', value: note?.date ?? defaultNoteDate() });
      const text = h('textarea', {
        class: 'textarea',
        name: 'body',
        maxlength: '10000',
        placeholder: 'Write down milestones or reflections',
        value: note?.body ?? '',
      });
      const save = h('button', { class: 'btn btn-primary', type: 'submit', text: 'Save' });
      const form = h(
        'form',
        { novalidate: true },
        error,
        field('Date', date),
        h('div', { class: 'field' }, h('label', { class: 'sr-only', for: 'note-body', text: 'Note' }), Object.assign(text, { id: 'note-body' })),
        h('div', { class: 'modal-actions' }, save, h('span', { class: 'hint', text: 'Ctrl/⌘ + Enter to save' })),
      );
      text.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) form.requestSubmit();
      });
      form.addEventListener('submit', async (event) => {
        event.preventDefault();
        const payload = { date: date.value, body: text.value.trim() };
        if (!payload.date) return showError(error, 'Pick a date for this note.');
        if (!payload.body) return showError(error, 'Write something before saving.'), text.focus();
        save.disabled = true;
        try {
          if (note) await api(`/notes/${note.id}`, { method: 'PATCH', body: payload });
          else await api('/notes', { method: 'POST', body: payload });
          close();
          if (monthOf(payload.date) !== state.month) await load(monthOf(payload.date));
          else await refresh();
        } catch (err) {
          showError(error, err.message);
          save.disabled = false;
        }
      });
      body.append(form);
      requestAnimationFrame(() => text.focus());
    },
  });
}

// ---- archived habits ----------------------------------------------------------
function openArchived() {
  openModal({
    title: 'Archived habits',
    content(body, close) {
      const list = h('ul', { class: 'list' }, h('li', { text: 'Loading…' }));
      body.append(h('p', { class: 'hint', text: 'Archived habits keep their history in past months. Restore one to bring it back to today.' }), list);
      const draw = async () => {
        try {
          const habits = await api('/habits/archived');
          if (!habits.length) {
            list.replaceChildren(h('li', { text: 'Nothing archived. Every habit is in play.' }));
            return;
          }
          list.replaceChildren(
            ...habits.map((x) =>
              h(
                'li',
                { class: `hc-${x.color}` },
                h(
                  'div',
                  { class: 'grow' },
                  h('span', { class: 'title' }, h('span', { class: 'name-text' }, h('span', { class: 'swatch' })), x.name),
                  h('span', {
                    class: 'meta',
                    text: `Archived ${noteDate(x.archivedAt.slice(0, 10))} · ${x.totalChecks} check-in${x.totalChecks === 1 ? '' : 's'}`,
                  }),
                ),
                h(
                  'button',
                  {
                    class: 'btn btn-sm btn-secondary',
                    type: 'button',
                    onclick: async () => {
                      try {
                        await api(`/habits/${x.id}`, { method: 'PATCH', body: { archived: false } });
                        toast(`“${x.name}” is back. Welcome back to it.`);
                        await Promise.all([draw(), refresh()]);
                      } catch (err) {
                        toast(err.message, { type: 'error' });
                      }
                    },
                  },
                  'Restore',
                ),
                armedButton({
                  label: 'Delete',
                  armedLabel: 'Confirm',
                  className: 'btn btn-sm btn-danger',
                  onConfirm: async () => {
                    try {
                      await api(`/habits/${x.id}`, { method: 'DELETE' });
                      await Promise.all([draw(), refresh()]);
                    } catch (err) {
                      toast(err.message, { type: 'error' });
                    }
                  },
                }),
              ),
            ),
          );
        } catch (err) {
          list.replaceChildren(h('li', { text: err.message }));
        }
      };
      draw();
      body.append(h('div', { class: 'modal-actions', style: { 'margin-top': '20px' } }, h('button', { class: 'btn btn-secondary', type: 'button', text: 'Done', onclick: close })));
    },
  });
}

// ---- settings -----------------------------------------------------------------
function describeAgent(ua = '') {
  const browser = /Edg\//.test(ua)
    ? 'Edge'
    : /Firefox\//.test(ua)
      ? 'Firefox'
      : /Chrome\//.test(ua)
        ? 'Chrome'
        : /Safari\//.test(ua)
          ? 'Safari'
          : 'Browser';
  const os = /iPhone|iPad/.test(ua)
    ? 'iOS'
    : /Android/.test(ua)
      ? 'Android'
      : /Mac OS X/.test(ua)
        ? 'macOS'
        : /Windows/.test(ua)
          ? 'Windows'
          : /Linux/.test(ua)
            ? 'Linux'
            : 'Unknown device';
  return `${browser} on ${os}`;
}

const ago = (ms) => {
  const minutes = Math.round((Date.now() - ms) / 60000);
  if (minutes < 2) return 'active now';
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  return `${Math.round(hours / 24)} d ago`;
};

function openSettings() {
  openModal({
    title: 'Settings',
    wide: true,
    content(body) {
      body.append(appearanceSection(), passwordSection(), twoFactorSection(), sessionsSection(), backupSection());
    },
  });
}

function appearanceSection() {
  const current = getThemePref();
  const group = h(
    'div',
    { class: 'order-row', role: 'radiogroup', 'aria-label': 'Theme' },
    ['system', 'light', 'dark'].map((value) => {
      const btn = h('button', {
        class: `btn btn-sm ${value === current ? 'btn-primary' : 'btn-secondary'}`,
        type: 'button',
        role: 'radio',
        'aria-checked': String(value === current),
        text: value[0].toUpperCase() + value.slice(1),
      });
      btn.addEventListener('click', () => {
        setThemePref(value);
        renderThemeButton();
        for (const b of group.children) {
          const on = b === btn;
          b.className = `btn btn-sm ${on ? 'btn-primary' : 'btn-secondary'}`;
          b.setAttribute('aria-checked', String(on));
        }
      });
      return btn;
    }),
  );
  return h('div', { class: 'section' }, h('h3', { text: 'Appearance' }), h('p', { text: 'Match your system or pick a side.' }), group);
}

function passwordSection() {
  const error = h('p', { class: 'form-error', role: 'alert', hidden: true });
  const ok = h('p', { class: 'form-success', role: 'status', hidden: true });
  const current = h('input', { class: 'input', type: 'password', autocomplete: 'current-password' });
  const next = h('input', { class: 'input', type: 'password', autocomplete: 'new-password' });
  const confirm = h('input', { class: 'input', type: 'password', autocomplete: 'new-password' });
  const save = h('button', { class: 'btn btn-primary btn-sm', type: 'submit', text: 'Update password' });
  const form = h(
    'form',
    { novalidate: true },
    error,
    ok,
    h('input', { type: 'text', autocomplete: 'username', value: state.me?.username ?? '', hidden: true, 'aria-hidden': 'true', tabindex: '-1' }),
    field('Current password', current),
    field('New password', next, { hint: 'At least 8 characters.' }),
    field('Confirm new password', confirm),
    save,
  );
  form.addEventListener('submit', async (event) => {
    event.preventDefault();
    showError(ok, '');
    if (next.value.length < 8) return showError(error, 'Use at least 8 characters.');
    if (next.value !== confirm.value) return showError(error, 'The new passwords don’t match yet.');
    save.disabled = true;
    try {
      const res = await api('/password', { method: 'POST', body: { current: current.value, next: next.value } });
      showError(error, '');
      form.reset();
      ok.textContent = res.signedOut
        ? `Password updated. ${res.signedOut} other session${res.signedOut === 1 ? ' was' : 's were'} signed out.`
        : 'Password updated.';
      ok.hidden = false;
    } catch (err) {
      showError(error, err.message);
    } finally {
      save.disabled = false;
    }
  });
  return h('div', { class: 'section' }, h('h3', { text: 'Password' }), h('p', { text: 'Changing it signs out every other device.' }), form);
}

function twoFactorSection() {
  const section = h('div', { class: 'section' });
  const draw = () => {
    const on = Boolean(state.me?.totpEnabled);
    section.replaceChildren(
      h(
        'h3',
        {},
        'Two-factor authentication ',
        h('span', { class: `status-pill ${on ? 'on' : 'off'}`, text: on ? 'On' : 'Off' }),
      ),
      h('p', {
        text: on
          ? 'Signing in needs your password and a 6-digit code from your authenticator app.'
          : 'Add a 6-digit code from an authenticator app on top of your password.',
      }),
      on ? disableTotpForm(draw) : h('button', { class: 'btn btn-sm btn-secondary', type: 'button', onclick: () => startTotp(section, draw) }, icon('shield'), 'Set up'),
    );
  };
  draw();
  return section;
}

async function startTotp(section, done) {
  let setup;
  try {
    setup = await api('/2fa/setup', { method: 'POST' });
  } catch (err) {
    toast(err.message, { type: 'error' });
    return;
  }
  const error = h('p', { class: 'form-error', role: 'alert', hidden: true });
  const code = h('input', { class: 'input code', inputmode: 'numeric', autocomplete: 'one-time-code', maxlength: '6', placeholder: '000000' });
  const grouped = setup.secret.match(/.{1,4}/g).join(' ');
  const copy = h('button', { class: 'icon-btn', type: 'button', 'aria-label': 'Copy key', title: 'Copy key' }, icon('copy'));
  copy.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(setup.secret);
      toast('Key copied.');
    } catch {
      toast('Select the key and copy it manually.', { type: 'error' });
    }
  });
  const enable = h('button', { class: 'btn btn-primary btn-sm', type: 'submit', text: 'Turn on' });
  const form = h(
    'form',
    { novalidate: true },
    h(
      'ol',
      { class: 'steps' },
      h('li', { text: 'Open your authenticator app and add a new account.' }),
      h('li', {}, 'Enter this setup key (time-based), or ', h('a', { href: setup.uri, text: 'open it directly on this device' }), '.'),
      h('li', { text: 'Type the 6-digit code it shows.' }),
    ),
    h('div', { class: 'secret-box' }, h('code', { text: grouped }), copy),
    error,
    field('Code', code),
    h('div', { class: 'modal-actions' }, enable, h('button', { class: 'btn btn-sm btn-secondary', type: 'button', text: 'Cancel', onclick: done })),
  );
  form.addEventListener('submit', async (event) => {
    event.preventDefault();
    enable.disabled = true;
    try {
      await api('/2fa/enable', { method: 'POST', body: { code: code.value } });
      state.me.totpEnabled = true;
      toast('Two-factor authentication is on. Your tracker is locked down.');
      done();
    } catch (err) {
      showError(error, err.message);
      enable.disabled = false;
    }
  });
  section.replaceChildren(h('h3', { text: 'Set up two-factor authentication' }), form);
  code.focus();
}

function disableTotpForm(done) {
  const error = h('p', { class: 'form-error', role: 'alert', hidden: true });
  const password = h('input', { class: 'input', type: 'password', autocomplete: 'current-password' });
  const code = h('input', { class: 'input code', inputmode: 'numeric', autocomplete: 'one-time-code', maxlength: '6', placeholder: '000000' });
  const submit = h('button', { class: 'btn btn-sm btn-danger', type: 'submit', text: 'Turn off' });
  const form = h('form', { novalidate: true }, error, field('Password', password), field('Current code', code), submit);
  form.hidden = true;
  const reveal = h('button', {
    class: 'btn btn-sm btn-secondary',
    type: 'button',
    text: 'Turn off…',
    onclick: () => {
      reveal.hidden = true;
      form.hidden = false;
      password.focus();
    },
  });
  form.addEventListener('submit', async (event) => {
    event.preventDefault();
    submit.disabled = true;
    try {
      await api('/2fa/disable', { method: 'POST', body: { password: password.value, code: code.value } });
      state.me.totpEnabled = false;
      toast('Two-factor authentication is off.');
      done();
    } catch (err) {
      showError(error, err.message);
      submit.disabled = false;
    }
  });
  return h('div', {}, reveal, form);
}

function sessionsSection() {
  const list = h('ul', { class: 'list' }, h('li', { text: 'Loading…' }));
  const draw = async () => {
    try {
      const sessions = await api('/sessions');
      list.replaceChildren(
        ...sessions.map((s) =>
          h(
            'li',
            {},
            h(
              'div',
              { class: 'grow' },
              h('span', { class: 'title' }, describeAgent(s.userAgent), s.current ? h('span', { class: 'status-pill on', text: 'This device' }) : null),
              h('span', { class: 'meta', text: `${s.current ? 'active now' : ago(s.lastSeen)}${s.ip ? ` · ${s.ip}` : ''}` }),
            ),
          ),
        ),
      );
    } catch (err) {
      list.replaceChildren(h('li', { text: err.message }));
    }
  };
  draw();
  const revoke = h('button', {
    class: 'btn btn-sm btn-secondary',
    type: 'button',
    text: 'Sign out all other devices',
    onclick: async () => {
      try {
        const { signedOut } = await api('/sessions/revoke-others', { method: 'POST' });
        toast(signedOut ? `Signed out ${signedOut} other session${signedOut === 1 ? '' : 's'}.` : 'No other devices were signed in.');
        draw();
      } catch (err) {
        toast(err.message, { type: 'error' });
      }
    },
  });
  return h(
    'div',
    { class: 'section' },
    h('h3', { text: 'Signed-in devices' }),
    h('p', { text: `Each device stays signed in for ${state.me?.sessionDays ?? 30} days after its last visit.` }),
    list,
    h('div', { class: 'modal-actions', style: { 'margin-top': '14px' } }, revoke),
  );
}

function backupSection() {
  return h(
    'div',
    { class: 'section' },
    h('h3', { text: 'Backup' }),
    h('p', { text: 'Download everything as JSON, or restore from a previous backup.' }),
    h(
      'div',
      { class: 'modal-actions' },
      h('a', { class: 'btn btn-sm btn-secondary', href: '/api/export', download: '' }, icon('download'), 'Export backup'),
      h('button', { class: 'btn btn-sm btn-secondary', type: 'button', onclick: () => els.importFile.click() }, icon('upload'), 'Import backup'),
    ),
  );
}

async function importBackup(file) {
  let data;
  try {
    data = JSON.parse(await file.text());
    if (!Array.isArray(data.habits) || !Array.isArray(data.notes)) throw new Error();
  } catch {
    toast('That file isn’t a Cadence backup.', { type: 'error' });
    return;
  }
  openModal({
    title: 'Restore backup?',
    content(body, close) {
      body.append(
        h('p', {
          text: `This replaces all current habits, check-ins and notes with ${data.habits.length} habit${data.habits.length === 1 ? '' : 's'} and ${data.notes.length} note${data.notes.length === 1 ? '' : 's'} from “${file.name}”.`,
        }),
        h(
          'div',
          { class: 'modal-actions' },
          h('button', {
            class: 'btn btn-primary',
            type: 'button',
            text: 'Replace and restore',
            onclick: async (event) => {
              event.currentTarget.disabled = true;
              try {
                await api('/import', { method: 'POST', body: data });
                close();
                toast('Backup restored.');
                await refresh();
              } catch (err) {
                toast(err.message, { type: 'error' });
                close();
              }
            },
          }),
          h('button', { class: 'btn btn-secondary', type: 'button', text: 'Cancel', onclick: close }),
        ),
      );
    },
  });
}

// ---- header: theme + user menu --------------------------------------------------
function renderThemeButton() {
  const dark = effectiveTheme() === 'dark';
  els.themeBtn.replaceChildren(icon(dark ? 'sun' : 'moon'));
  els.themeBtn.setAttribute('aria-label', dark ? 'Switch to light mode' : 'Switch to dark mode');
  els.themeBtn.title = els.themeBtn.getAttribute('aria-label');
}

function buildMenu() {
  const item = (iconName, label, onclick) =>
    h('button', { type: 'button', role: 'menuitem', onclick: () => (closeMenu(), onclick()) }, icon(iconName), label);
  els.userMenu.replaceChildren(
    h('div', { class: 'menu-label', text: `Signed in as ${state.me.username}` }),
    item('archive', 'Archived habits', openArchived),
    item('settings', 'Settings & security', openSettings),
    h('a', { href: '/api/export', download: '', role: 'menuitem', onclick: closeMenu }, icon('download'), 'Export backup'),
    item('upload', 'Import backup', () => els.importFile.click()),
    h('hr'),
    item('logout', 'Sign out', async () => {
      try {
        await api('/logout', { method: 'POST' });
      } finally {
        location.replace('/login');
      }
    }),
  );
}

function openMenu() {
  els.userMenu.hidden = false;
  els.userBtn.setAttribute('aria-expanded', 'true');
  els.userMenu.querySelector('[role="menuitem"]')?.focus();
}

function closeMenu() {
  els.userMenu.hidden = true;
  els.userBtn.setAttribute('aria-expanded', 'false');
}

// ---- boot ---------------------------------------------------------------------
function wireEvents() {
  els.prev.addEventListener('click', () => load(shiftMonth(state.month, -1)));
  els.next.addEventListener('click', () => load(shiftMonth(state.month, 1)));
  els.todayBtn.addEventListener('click', () => load(currentMonth()));
  els.newNote.addEventListener('click', () => openNoteModal());
  els.themeBtn.addEventListener('click', () => {
    setThemePref(effectiveTheme() === 'dark' ? 'light' : 'dark');
    renderThemeButton();
  });
  matchMedia('(prefers-color-scheme: dark)').addEventListener('change', renderThemeButton);

  els.userBtn.addEventListener('click', () => (els.userMenu.hidden ? openMenu() : closeMenu()));
  document.addEventListener('click', (e) => {
    if (!els.userMenu.hidden && !e.target.closest('.menu-wrap')) closeMenu();
  });
  els.userMenu.addEventListener('keydown', (e) => {
    const items = [...els.userMenu.querySelectorAll('[role="menuitem"]')];
    const i = items.indexOf(document.activeElement);
    if (e.key === 'ArrowDown') items[(i + 1) % items.length].focus();
    else if (e.key === 'ArrowUp') items[(i - 1 + items.length) % items.length].focus();
    else if (e.key === 'Escape') {
      closeMenu();
      els.userBtn.focus();
    } else return;
    e.preventDefault();
  });

  els.importFile.addEventListener('change', () => {
    const [file] = els.importFile.files;
    els.importFile.value = '';
    if (file) importBackup(file);
  });

  document.addEventListener('keydown', (e) => {
    if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey || document.querySelector('dialog[open]')) return;
    if (e.target.closest('input, textarea, select, [contenteditable]')) return;
    if (e.key === 'ArrowLeft') load(shiftMonth(state.month, -1));
    else if (e.key === 'ArrowRight') load(shiftMonth(state.month, 1));
    else if (e.key === 't' || e.key === 'T') load(currentMonth());
    else if (e.key === 'n' || e.key === 'N') openHabitModal();
  });

  window.addEventListener('hashchange', () => {
    const month = location.hash.slice(1);
    if (isMonthKey(month) && month !== state.month) load(month);
  });

  // Keep "today" right when the tab stays open past midnight.
  const syncToday = () => {
    const today = ymd(new Date());
    if (today !== state.today) {
      state.today = today;
      render({ scrollToToday: false });
    }
  };
  document.addEventListener('visibilitychange', () => document.visibilityState === 'visible' && syncToday());
  setInterval(syncToday, 60_000);
}

async function boot() {
  renderThemeButton();
  wireEvents();
  const fromHash = location.hash.slice(1);
  const month = isMonthKey(fromHash) ? fromHash : currentMonth();
  const [me] = await Promise.all([api('/me').catch(() => null), load(month)]);
  if (!me) return;
  state.me = me;
  els.username.textContent = me.username;
  els.avatar.textContent = me.username.slice(0, 1);
  buildMenu();
}

boot();
