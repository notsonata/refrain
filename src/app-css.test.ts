import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

describe('compact card overflow menus', () => {
  it('drops content visibility containment while a card menu is open', () => {
    const css = readFileSync(new URL('./app.css', import.meta.url), 'utf8');

    expect(css).toMatch(
      /\.track-card:has\(\.overflow-menu\)\s*\{[^}]*content-visibility:\s*visible;/s,
    );
  });
});

describe('sidebar vertical containment', () => {
  it('keeps footer controls visible by scrolling navigation when needed', () => {
    const css = readFileSync(new URL('./app.css', import.meta.url), 'utf8');

    expect(css).toMatch(
      /\.sidebar-nav\s*\{[^}]*min-height:\s*0;[^}]*overflow-y:\s*auto;/s,
    );
    expect(css).toMatch(/\.sidebar-footer\s*\{[^}]*flex:\s*0 0 auto;/s);
  });

  it('uses the taller desktop window baseline', () => {
    const config = JSON.parse(
      readFileSync(
        new URL('../src-tauri/tauri.conf.json', import.meta.url),
        'utf8',
      ),
    );
    const mainWindow = config.app.windows[0];

    expect(mainWindow.height).toBe(780);
    expect(mainWindow.minHeight).toBe(720);
  });
});

describe('desktop text selection', () => {
  it('prevents selecting application chrome while preserving text-entry selection', () => {
    const css = readFileSync(new URL('./app.css', import.meta.url), 'utf8');

    expect(css).toMatch(
      /\.app-shell\s*\{[^}]*-webkit-user-select:\s*none;[^}]*user-select:\s*none;/s,
    );
    expect(css).toMatch(
      /\.app-shell\s+:is\(input,\s*textarea,\s*\[contenteditable=['"]true['"]\],\s*\[contenteditable=['"]['"]\]\)\s*\{[^}]*-webkit-user-select:\s*text;[^}]*user-select:\s*text;/s,
    );
    expect(css).not.toMatch(
      /\.sidebar-feedback-modal-message\s*\{[^}]*user-select:\s*text;/s,
    );
  });
});

describe('staging warning sizing', () => {
  it('keeps staging errors content-sized instead of filling the workspace', () => {
    const css = readFileSync(
      new URL('./components/StagingView.svelte', import.meta.url),
      'utf8',
    );

    expect(css).toMatch(
      /\.staging-error\s*\{[^}]*flex:\s*0 0 auto;[^}]*min-height:\s*0;/s,
    );
  });
});

describe('staging selection styling', () => {
  it('keeps the selected track card neutral instead of filling it blue', () => {
    const css = readFileSync(
      new URL('./components/StagingView.svelte', import.meta.url),
      'utf8',
    );
    const selectedCard = css.match(
      /\.download-card\.selected-card\s*\{([^}]*)\}/s,
    )?.[1];

    expect(selectedCard).toContain('background: var(--bg-subtle);');
    expect(selectedCard).not.toMatch(/blue/i);
  });
});
