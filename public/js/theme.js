// Runs before first paint so a saved light/dark preference never flashes.
try {
  const theme = localStorage.getItem('cadence-theme');
  if (theme === 'light' || theme === 'dark') document.documentElement.dataset.theme = theme;
} catch {
  // storage unavailable: follow the system theme
}
