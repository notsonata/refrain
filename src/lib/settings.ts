import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './app-info';

export interface AppSettings {
  libraryRoot: string | null;
  keepRemovedManagedFiles: boolean;
  syncOnStartup: boolean;
  syncIntervalMinutes: number | null;
  acquisitionEnabled: boolean;
}

export function getSettings(invokeFn: InvokeFn = invoke): Promise<AppSettings> {
  return invokeFn<AppSettings>('get_settings');
}

export function updateSettings(
  settings: AppSettings,
  invokeFn: InvokeFn = invoke,
): Promise<AppSettings> {
  return invokeFn<AppSettings>('update_settings', { settings });
}
