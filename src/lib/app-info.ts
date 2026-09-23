import { invoke } from '@tauri-apps/api/core';

export interface AppInfo {
  name: string;
  version: string;
  dataDir: string;
}

export type InvokeFn = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export function getAppInfo(invokeFn: InvokeFn = invoke): Promise<AppInfo> {
  return invokeFn<AppInfo>('get_app_info');
}
