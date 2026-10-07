import { describe, expect, it, vi } from 'vitest';
import type { InvokeFn } from './app-info';
import {
  clearAntraDeviceToken,
  getAntraAccountStatus,
  pollAntraDeviceLogin,
  startAntraDeviceLogin,
} from './antra';

describe('Antra commands', () => {
  it('starts device login and polls with the returned device code', async () => {
    const invoke = vi.fn(
      async <T>(command: string, args?: Record<string, unknown>) => {
        if (command === 'start_antra_device_login') {
          return {
            deviceCode: 'device-123',
            userCode: 'ABCD-EFGH',
            verificationUrl: 'https://antra.hoshi.cfd/device',
            expiresIn: 600,
            interval: 5,
          } as T;
        }
        expect(command).toBe('poll_antra_device_login');
        expect(args).toEqual({ deviceCode: 'device-123' });
        return { status: 'pending', configured: false } as T;
      },
    ) as InvokeFn;

    const started = await startAntraDeviceLogin(invoke);
    expect(started.userCode).toBe('ABCD-EFGH');
    await expect(
      pollAntraDeviceLogin(started.deviceCode, invoke),
    ).resolves.toMatchObject({
      status: 'pending',
    });
  });

  it('loads and clears account status', async () => {
    const invoke = vi.fn(async <T>(command: string) => {
      expect([
        'get_antra_account_status',
        'clear_antra_device_token',
      ]).toContain(command);
      return { configured: command === 'get_antra_account_status' } as T;
    }) as InvokeFn;

    await expect(getAntraAccountStatus(invoke)).resolves.toEqual({
      configured: true,
    });
    await expect(clearAntraDeviceToken(invoke)).resolves.toEqual({
      configured: false,
    });
  });
});
