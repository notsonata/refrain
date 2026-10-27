import { readFileSync } from 'node:fs';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { ComponentProps } from 'svelte';
import SettingsView from './SettingsView.svelte';

const settingsSource = readFileSync(
  new URL('./SettingsView.svelte', import.meta.url),
  'utf8',
);

describe('SettingsView tab isolation', () => {
  it('renders only General settings when Settings opens', () => {
    const props = {
      appInfo: { name: 'Refrain', version: '0.6.0', dataDir: '/app/data' },
      authStatus: null,
      sourceOverview: null,
      libraryOverview: null,
      savedAlbumTotal: 0,
      acquisitionEnabled: false,
      acquisitionProviders: ['monochrome'],
      themePreference: 'system',
    } as unknown as ComponentProps<typeof SettingsView>;
    const { body } = render(SettingsView, { props });

    expect(body).toContain('id="settings-panel-general"');
    for (const tab of ['sync', 'spotify', 'library', 'advanced']) {
      expect(body).not.toContain(`id="settings-panel-${tab}"`);
    }
    expect(body).toContain('Appearance');
    expect(body).not.toContain('Application runtime');
    expect(body).not.toContain('Spotify account');
    expect(body).not.toContain('Settings summary');
  });

  it('keeps source refresh exclusively in the Sync tab', () => {
    const syncPanel = settingsSource
      .split("{:else if activeTab === 'sync'}")[1]
      .split("{:else if activeTab === 'spotify'}")[0];
    const spotifyPanel = settingsSource
      .split("{:else if activeTab === 'spotify'}")[1]
      .split("{:else if activeTab === 'library'}")[0];

    expect(syncPanel).toContain('onclick={onRefreshSpotify}');
    expect(syncPanel).toContain('{#if sourceError}');
    expect(syncPanel).toContain('{#if sourceSummary}');
    expect(spotifyPanel).not.toContain('onclick={onRefreshSpotify}');
    expect(spotifyPanel).not.toContain('{#if sourceError}');
    expect(spotifyPanel).not.toContain('{#if sourceSummary}');
    expect(settingsSource).not.toContain('settings-summary-rail');
  });

  it('keeps destination controls out of Settings Library', () => {
    expect(settingsSource).not.toContain('MirrorSettings');
    expect(settingsSource).not.toContain('Playlist storage');
  });
});
