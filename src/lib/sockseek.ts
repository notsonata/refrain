import { invoke } from '@tauri-apps/api/core';

export interface SoulseekCredentialStatus {
  configured: boolean;
  username: string | null;
}

export interface ProviderHealth {
  available: boolean;
  version: string | null;
  message: string | null;
}

const providerHealthStorageKey = 'refrain-sockseek-health';

export function getCachedSockseekProviderHealth(): ProviderHealth | null {
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

export function cacheSockseekProviderHealth(health: ProviderHealth) {
  window.localStorage.setItem(providerHealthStorageKey, JSON.stringify(health));
}

export function clearCachedSockseekProviderHealth() {
  window.localStorage.removeItem(providerHealthStorageKey);
}

export function getSoulseekCredentialStatus(): Promise<SoulseekCredentialStatus> {
  return invoke('get_soulseek_credential_status');
}

export function setSoulseekCredentials(
  username: string,
  password: string,
): Promise<SoulseekCredentialStatus> {
  return invoke('set_soulseek_credentials', { username, password });
}

export function clearSoulseekCredentials(): Promise<SoulseekCredentialStatus> {
  return invoke('clear_soulseek_credentials');
}

export function getSockseekProviderHealth(): Promise<ProviderHealth> {
  return invoke('get_sockseek_provider_health');
}
