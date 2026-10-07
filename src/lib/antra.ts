import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './app-info';
import type { ProviderHealth } from './monochrome';

export interface AntraAccountStatus {
  configured: boolean;
}

export interface AntraDeviceCode {
  deviceCode: string;
  userCode: string;
  verificationUrl: string;
  expiresIn: number;
  interval: number;
}

export interface AntraDeviceLoginStatus {
  status: string;
  configured: boolean;
  username: string | null;
  tier: string | null;
  error: string | null;
}

const providerHealthStorageKey = 'refrain-antra-health';

export function getCachedAntraProviderHealth(): ProviderHealth | null {
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

export function cacheAntraProviderHealth(health: ProviderHealth) {
  window.localStorage.setItem(providerHealthStorageKey, JSON.stringify(health));
}

export function clearCachedAntraProviderHealth() {
  window.localStorage.removeItem(providerHealthStorageKey);
}

export function getAntraAccountStatus(
  invokeFn: InvokeFn = invoke,
): Promise<AntraAccountStatus> {
  return invokeFn<AntraAccountStatus>('get_antra_account_status');
}

export function startAntraDeviceLogin(
  invokeFn: InvokeFn = invoke,
): Promise<AntraDeviceCode> {
  return invokeFn<AntraDeviceCode>('start_antra_device_login');
}

export function pollAntraDeviceLogin(
  deviceCode: string,
  invokeFn: InvokeFn = invoke,
): Promise<AntraDeviceLoginStatus> {
  return invokeFn<AntraDeviceLoginStatus>('poll_antra_device_login', {
    deviceCode,
  });
}

export function clearAntraDeviceToken(
  invokeFn: InvokeFn = invoke,
): Promise<AntraAccountStatus> {
  return invokeFn<AntraAccountStatus>('clear_antra_device_token');
}

export function getAntraProviderHealth(
  invokeFn: InvokeFn = invoke,
): Promise<ProviderHealth> {
  return invokeFn<ProviderHealth>('get_antra_provider_health');
}

export function openAntraVerificationUrl(
  url: string,
  invokeFn: InvokeFn = invoke,
): Promise<void> {
  return invokeFn<void>('open_external_url', { url });
}
