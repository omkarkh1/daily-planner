import { api, showError } from './ui.js';

const form = document.getElementById('login-form');
const error = document.getElementById('login-error');
const submit = document.getElementById('login-submit');
const codeField = document.getElementById('code-field');
const code = document.getElementById('code');

document.getElementById('username').focus();

form.addEventListener('submit', async (event) => {
  event.preventDefault();
  const data = Object.fromEntries(new FormData(form));
  if (!data.username || !data.password) {
    showError(error, 'Enter your username and password.');
    return;
  }
  submit.disabled = true;
  submit.textContent = 'Signing in…';
  try {
    await api('/login', {
      method: 'POST',
      body: { username: data.username, password: data.password, code: codeField.hidden ? undefined : data.code },
      redirectOn401: false,
    });
    location.replace('/');
  } catch (err) {
    if (err.data?.totpRequired && codeField.hidden) {
      codeField.hidden = false;
      showError(error, '');
      code.focus();
    } else {
      showError(error, err.message);
      if (err.data?.totpRequired) {
        code.value = '';
        code.focus();
      }
    }
    submit.disabled = false;
    submit.textContent = 'Sign in';
  }
});

// Submit automatically once all six digits are typed.
code.addEventListener('input', () => {
  code.value = code.value.replace(/\D/g, '').slice(0, 6);
  if (code.value.length === 6) form.requestSubmit();
});
