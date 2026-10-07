import { invoke } from '@tauri-apps/api/core';

export interface ProviderHealth {
  available: boolean;
  version: string | null;
  message: string | null;
}

const providerHealthStorageKey = 'refrain-monochrome-health';

export function getCachedMonochromeProviderHealth(): ProviderHealth | null {
  try {
    const raw = window.localStorage.getItem(providerHealthStorageKey);
    if (!raw) return null;
    const value = JSON.parse(raw) as Partial<ProviderHealth>;
    if (typeof value.available !== 'boolean') return null;
    return {
      available: value.available,
      version: typeof value.version === 'string' ? value.version : null,
      message: typeof value.message === 'string' ? value.message : null,
    };
  } catch {
    return null;
  }
}

export function cacheMonochromeProviderHealth(health: ProviderHealth) {
  window.localStorage.setItem(providerHealthStorageKey, JSON.stringify(health));
}

export function clearCachedMonochromeProviderHealth() {
  window.localStorage.removeItem(providerHealthStorageKey);
}

export function getMonochromeProviderHealth(): Promise<ProviderHealth> {
  return invoke('get_monochrome_provider_health');
}
