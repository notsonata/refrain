import { isTauri } from '@tauri-apps/api/core';
import { Effect, EffectState, getCurrentWindow } from '@tauri-apps/api/window';

export type ThemePreference = 'system' | 'light' | 'dark';

const storageKey = 'refrain-theme';
const darkQuery = '(prefers-color-scheme: dark)';

async function syncNativeWindowTheme(preference: ThemePreference) {
  if (!isTauri()) return;

  const window = getCurrentWindow();
  try {
    await window.setTheme(preference === 'system' ? null : preference);

    const platform = document.documentElement.dataset.platform;
    if (platform === 'macos') {
      await window.setEffects({
        effects: [Effect.Sidebar],
        state: EffectState.FollowsWindowActiveState,
      });
    } else if (platform === 'windows') {
      await window.setEffects({ effects: [Effect.Acrylic] });
    }
  } catch {
    // Native appearance is best-effort; the CSS theme remains authoritative.
  }
}

export function getThemePreference(): ThemePreference {
  const stored = window.localStorage.getItem(storageKey);
  if (stored === 'system' || stored === 'light' || stored === 'dark') {
    return stored;
  }
  return 'light';
}

export function applyThemePreference(preference: ThemePreference) {
  const dark =
    preference === 'dark' ||
    (preference === 'system' && window.matchMedia(darkQuery).matches);
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
  void syncNativeWindowTheme(preference);
}

export function setThemePreference(preference: ThemePreference) {
  window.localStorage.setItem(storageKey, preference);
  applyThemePreference(preference);
}

export function initializeTheme() {
  const media = window.matchMedia(darkQuery);
  const applyStored = () => applyThemePreference(getThemePreference());
  applyStored();
  media.addEventListener('change', applyStored);
}
