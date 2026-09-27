import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';
import { initializeTheme } from './lib/theme';

const platform = navigator.platform.toLowerCase();
document.documentElement.dataset.platform = platform.includes('mac')
  ? 'macos'
  : platform.includes('win')
    ? 'windows'
    : platform.includes('linux')
      ? 'linux'
      : 'unknown';

initializeTheme();

const target = document.getElementById('app');

if (!target) {
  throw new Error('Missing application mount point');
}

mount(App, { target });
