import { describe, expect, it, vi } from 'vitest';
import type { InvokeFn } from './app-info';
import { getSettings, updateSettings, type AppSettings } from './settings';

const settings: AppSettings = {
  libraryRoot: null,
  keepRemovedManagedFiles: true,
  syncOnStartup: false,
  syncIntervalMinutes: null,
  acquisitionEnabled: false,
};

describe('settings commands', () => {
  it('loads settings through a use-case command', async () => {
    const invoke = vi.fn(async <T>(command: string) => {
      expect(command).toBe('get_settings');
      return settings as T;
    }) as InvokeFn;

    await expect(getSettings(invoke)).resolves.toEqual(settings);
  });

  it('updates settings through a use-case command', async () => {
    const invoke = vi.fn(
      async <T>(command: string, args?: Record<string, unknown>) => {
        expect(command).toBe('update_settings');
        expect(args).toEqual({ settings });
        return settings as T;
      },
    ) as InvokeFn;

    await expect(updateSettings(settings, invoke)).resolves.toEqual(settings);
  });
});
