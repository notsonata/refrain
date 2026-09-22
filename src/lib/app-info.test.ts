import { describe, expect, it, vi } from 'vitest';
import { getAppInfo, type AppInfo, type InvokeFn } from './app-info';

describe('getAppInfo', () => {
  it('requests application information from the Tauri backend', async () => {
    const expected: AppInfo = {
      name: 'Refrain',
      version: '0.1.0',
      dataDir: '/tmp/refrain',
    };

    const invoke = vi.fn(async <T>(command: string) => {
      expect(command).toBe('get_app_info');
      return expected as T;
    }) as InvokeFn;

    await expect(getAppInfo(invoke)).resolves.toEqual(expected);
    expect(invoke).toHaveBeenCalledOnce();
  });
});
