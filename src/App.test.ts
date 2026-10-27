import { readFileSync } from 'node:fs';
import { render } from 'svelte/server';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from './App.svelte';

const appSource = readFileSync(
  new URL('./App.svelte', import.meta.url),
  'utf8',
);
const mirrorSource = readFileSync(
  new URL('./components/MirrorSettings.svelte', import.meta.url),
  'utf8',
);

describe('Local mirror navigation and feedback', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('shows Mirrors under Local in the persistent sidebar', () => {
    vi.stubGlobal('window', {
      localStorage: { getItem: () => null },
    });
    const { body } = render(App);
    expect(body).toMatch(/<span>Playlists<\/span>[\s\S]*<span>Mirrors<\/span>/);
    const localDropdown = appSource.match(
      /\{#if localExpanded\}([\s\S]*?)\{\/if\}/,
    )?.[1];
    expect(localDropdown).toContain("openLocalSection('mirrors')");
    expect(localDropdown).toContain("localSection === 'mirrors'");
    expect(localDropdown).toMatch(
      /<span>Playlists<\/span>[\s\S]*<span>Mirrors<\/span>/,
    );
    expect(appSource.match(/openLocalSection\('mirrors'\)/g)).toHaveLength(1);
  });

  it('keeps mirror run progress and its result in the app shell', () => {
    expect(appSource).toContain(
      "listen<MirrorProgress>('library-mirror-progress'",
    );
    expect(appSource).toContain('sidebarFeedbackIndex = 0;');
    expect(appSource).toContain('Library mirror complete');
    expect(appSource).toContain('Library mirror failed');
    expect(appSource).toContain('mirrorRunRevision += 1;');
    expect(mirrorSource).not.toContain('listen<MirrorProgress>');
    expect(mirrorSource).not.toContain('runMirror(');
  });
});
