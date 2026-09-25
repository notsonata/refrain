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
