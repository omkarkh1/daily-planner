import { api, showError } from './ui.js';

const form = document.getElementById('setup-form');
const error = document.getElementById('setup-error');
const submit = document.getElementById('setup-submit');
const token = document.getElementById('token');

const fromUrl = new URLSearchParams(location.search).get('token');
if (fromUrl) {
  token.value = fromUrl;
  document.getElementById('token-field').hidden = true;
  document.getElementById('username').focus();
} else {
  token.focus();
}

form.addEventListener('submit', async (event) => {
  event.preventDefault();
  const data = Object.fromEntries(new FormData(form));
  if (!data.token.trim()) return showError(error, 'Paste the setup token from the server log.');
  if (!data.username.trim()) return showError(error, 'Choose a username.');
  if (data.password.length < 8) return showError(error, 'Use at least 8 characters for your password.');
  if (data.password !== data.confirm) return showError(error, 'The two passwords don’t match yet.');

  submit.disabled = true;
  submit.textContent = 'Creating…';
  try {
    await api('/setup', {
      method: 'POST',
      body: { token: data.token, username: data.username, password: data.password },
      redirectOn401: false,
    });
    location.replace('/');
  } catch (err) {
    if (err.status === 403) document.getElementById('token-field').hidden = false;
    if (err.status === 409) location.replace('/login');
    showError(error, err.message);
    submit.disabled = false;
    submit.textContent = 'Create account';
  }
});
