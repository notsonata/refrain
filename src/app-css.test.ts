import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const appCss = readFileSync(new URL('./app.css', import.meta.url), 'utf8');

function themeTokens(selector: string): Record<string, string> {
  const block = appCss.match(
    new RegExp(
      `${selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\s*\\{([^}]*)\\}`,
    ),
  )?.[1];
  if (!block) throw new Error(`Missing ${selector} theme block`);

  return Object.fromEntries(
    Array.from(
      block.matchAll(/(--[\w-]+):\s*(#[\da-f]{6});/gi),
      ([, name, value]) => [name, value],
    ),
  );
}

function contrast(foreground: string, background: string): number {
  const luminance = (hex: string) => {
    const channels = hex
      .slice(1)
      .match(/.{2}/g)!
      .map((channel) => {
        const value = parseInt(channel, 16) / 255;
        return value <= 0.04045
          ? value / 12.92
          : ((value + 0.055) / 1.055) ** 2.4;
      });
    return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
  };

  const a = luminance(foreground);
  const b = luminance(background);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

describe('semantic control contrast', () => {
  const lightTokens = themeTokens(':root');
  const darkTokens = {
    ...lightTokens,
    ...themeTokens("html[data-theme='dark']"),
  };

  for (const [mode, tokens] of [
    ['light', lightTokens],
    ['dark', darkTokens],
  ] as const) {
    it(`${mode} theme keeps action, status, and disabled labels at 4.5:1 contrast`, () => {
      const pairs = [
        ['--action-primary-text', '--action-primary-bg'],
        ['--action-primary-text', '--action-primary-hover'],
        ['--action-primary-text', '--control-accent-bg'],
        ['--action-disabled-text', '--action-disabled-bg'],
        ['--text-disabled', '--control-disabled-bg'],
        ['--green-600', '--green-100'],
        ['--orange-600', '--orange-100'],
        ['--red-600', '--red-100'],
        ['--blue-600', '--blue-100'],
        ['--blue-700', '--blue-100'],
        ['--blue-600', '--blue-050'],
        ['--blue-700', '--blue-050'],
        ['--text-primary', '--bg-elevated'],
        ['--text-secondary', '--bg-subtle'],
        ['--text-tertiary', '--bg-content'],
      ];

      for (const [text, surface] of pairs) {
        expect(
          contrast(tokens[text], tokens[surface]),
          `${mode} ${text} on ${surface}`,
        ).toBeGreaterThanOrEqual(4.5);
      }
    });
  }

  it('uses text-safe primary action tokens in Settings instead of the light blue accent', () => {
    const settingsCss = readFileSync(
      new URL('./components/SettingsView.svelte', import.meta.url),
      'utf8',
    );
    const primaryRule = settingsCss.match(
      /\.primary-button\s*\{([^}]*)\}/s,
    )?.[1];

    expect(primaryRule).toContain('background: var(--action-primary-bg);');
    expect(primaryRule).toContain('color: var(--action-primary-text);');
    expect(primaryRule).not.toContain('var(--blue-600)');
  });

  it('keeps sync prominent and refresh accessible in a compact footer', () => {
    const app = readFileSync(new URL('./App.svelte', import.meta.url), 'utf8');
    expect(app).toMatch(
      /class="sidebar-action-button primary"[\s\S]*?Sync Library[\s\S]*?class="sidebar-action-button sidebar-refresh-button"/,
    );
    expect(app).toContain(
      "aria-label={sourceBusy ? 'Refreshing Spotify' : 'Refresh Spotify'}",
    );
    expect(app).toContain('Last refresh ${formatSyncTime(');
    expect(app).not.toContain('class="sidebar-status"');
    expect(appCss).toMatch(
      /\.sidebar-sync-actions\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\) 36px;/s,
    );
  });

  it('keeps rejected cards and cached notices legible without opacity', () => {
    const settingsCss = readFileSync(
      new URL('./components/SettingsView.svelte', import.meta.url),
      'utf8',
    );
    expect(
      appCss.match(/\.candidate-card\.rejected\s*\{([^}]*)\}/s)?.[1],
    ).not.toContain('opacity:');
    expect(
      settingsCss.match(/\.cached-status\s*\{([^}]*)\}/s)?.[1],
    ).not.toContain('opacity:');
    expect(settingsCss).toMatch(/\.settings-panel:focus-visible\s*\{/);
  });

  it('uses the contrast-safe semantic palette for the account avatar fallback', () => {
    expect(appCss).toMatch(
      /\.account-avatar\s*\{[^}]*background:\s*var\(--blue-100\);[^}]*color:\s*var\(--blue-700\);/s,
    );
  });

  it('uses high-contrast filled switch colors and a defined focus-ring token', () => {
    const settingsCss = readFileSync(
      new URL('./components/SettingsView.svelte', import.meta.url),
      'utf8',
    );

    expect(appCss).toMatch(
      /\.tracking-switch\.active\s*\{[^}]*background:\s*var\(--control-accent-bg\)/s,
    );
    expect(settingsCss).toMatch(
      /\.switch-row input\[type='checkbox'\]:checked\s*\{[^}]*background:\s*var\(--control-accent-bg\)/s,
    );
    expect(settingsCss).toMatch(
      /:global\(html\[data-theme='dark'\]\)\s+\.switch-row\s+input\[type='checkbox'\]:disabled::after\s*\{[^}]*background:\s*var\(--action-disabled-text\)/s,
    );
    expect(appCss).not.toContain('var(--blue-300)');
  });
});

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

describe('Spotify status column', () => {
  it('allows long labels to wrap inside their grid cell', () => {
    expect(appCss).toMatch(
      /\.spotify-track-table \.state-text\s*\{[^}]*max-width:\s*100%;/s,
    );
    expect(appCss).toMatch(
      /\.spotify-track-table \.state-label\s*\{[^}]*white-space:\s*normal;/s,
    );
  });
});

describe('Local Library table density', () => {
  it('reduces only the Local table grid spacing and clips Status labels', () => {
    expect(appCss).toMatch(
      /\.local-library-table \.table-head,\s*\.local-library-table \.table-row\s*\{\s*column-gap:\s*8px;/s,
    );
    expect(appCss).toMatch(
      /\.local-library-table \.state-text\s*\{[^}]*overflow:\s*hidden;/s,
    );
    expect(appCss).toMatch(
      /\.local-library-table \.state-label\s*\{[^}]*text-overflow:\s*ellipsis;/s,
    );
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
