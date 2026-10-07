<script lang="ts">
  import Icon from './Icon.svelte';
  import type { AppInfo } from '../lib/app-info';
  import type {
    LocalLibraryOverview,
    LocalLibraryScanProgress,
    LocalLibraryScanSummary,
  } from '../lib/library';
  import type { ProviderHealth } from '../lib/monochrome';
  import type { AcquisitionProviderId } from '../lib/settings';
  import type { SpotifySourceOverview } from '../lib/source';
  import type {
    SpotifyAuthStatus,
    SpotifySourceRefreshSummary,
  } from '../lib/spotify';
  import type { SyncRun } from '../lib/sync';
  import type { ThemePreference } from '../lib/theme';

  type SettingsTab = 'general' | 'sync' | 'spotify' | 'library' | 'advanced';

  type VoidAction = () => void | Promise<void>;
  type AcquisitionSettingsAction = (
    enabled: boolean,
    providers: AcquisitionProviderId[],
  ) => void | Promise<void>;
  type ThemeAction = (preference: ThemePreference) => void;

  export let appInfo: AppInfo | null;
  export let authStatus: SpotifyAuthStatus | null;
  export let clientId: string;
  export let authBusy: boolean;
  export let authError: string | null;
  export let sourceBusy: boolean;
  export let sourceError: string | null;
  export let sourceSummary: SpotifySourceRefreshSummary | null;
  export let sourceOverview: SpotifySourceOverview | null;
  export let savedAlbumTotal: number;

  export let libraryRoot: string;
  export let libraryBusy: boolean;
  export let libraryError: string | null;
  export let libraryProgress: LocalLibraryScanProgress | null;
  export let librarySummary: LocalLibraryScanSummary | null;
  export let libraryOverview: LocalLibraryOverview | null;

  export let acquisitionEnabled: boolean;
  export let acquisitionProviders: AcquisitionProviderId[];
  export let soulseekUsername: string;
  export let soulseekPassword: string;
  export let soulseekConfigured: boolean;
  export let monochromeHealth: ProviderHealth | null;
  export let monochromeHealthCached: boolean;
  export let antraConfigured: boolean;
  export let antraLoginPending: boolean;
  export let antraUserCode: string | null;
  export let antraVerificationUrl: string | null;
  export let antraHealth: ProviderHealth | null;
  export let antraHealthCached: boolean;
  export let sockseekHealth: ProviderHealth | null;
  export let sockseekHealthCached: boolean;
  export let acquisitionBusy: boolean;
  export let acquisitionError: string | null;

  export let syncBusy: boolean;
  export let syncScope: 'local' | 'spotify' | null;
  export let syncError: string | null;
  export let localSyncRun: SyncRun | null;
  export let spotifySyncRun: SyncRun | null;
  export let recentSyncRuns: SyncRun[];
  export let themePreference: ThemePreference;

  export let onConnectSpotify: VoidAction;
  export let onDisconnectSpotify: VoidAction;
  export let onRefreshSpotify: VoidAction;
  export let onSyncLibrary: VoidAction;
  export let onPickLibraryRoot: VoidAction;
  export let onSaveLibraryRoot: VoidAction;
  export let onScanLibrary: VoidAction;
  export let onSaveAcquisition: AcquisitionSettingsAction;
  export let onCheckMonochrome: VoidAction;
  export let onCheckAntra: VoidAction;
  export let onStartAntra: VoidAction;
  export let onOpenAntraLogin: VoidAction;
  export let onCancelAntra: VoidAction;
  export let onClearAntra: VoidAction;
  export let onSaveSoulseek: VoidAction;
  export let onCheckSockseek: VoidAction;
  export let onClearSoulseek: VoidAction;
  export let onChangeTheme: ThemeAction;

  const fallbackRedirectUri = 'http://127.0.0.1:43817/callback';
  const tabs = [
    {
      id: 'general',
      label: 'General',
      icon: 'settings',
      description: 'Overview and app status',
    },
    {
      id: 'sync',
      label: 'Sync',
      icon: 'sync',
      description: 'Library synchronization',
    },
    {
      id: 'spotify',
      label: 'Spotify',
      icon: 'spotify',
      description: 'Account and source data',
    },
    {
      id: 'library',
      label: 'Library',
      icon: 'local',
      description: 'Local music index',
    },
    {
      id: 'advanced',
      label: 'Advanced',
      icon: 'sliders',
      description: 'Runtime information',
    },
  ] as const;

  let activeTab: SettingsTab = 'general';

  const acquisitionProviderOptions: Array<{
    id: AcquisitionProviderId;
    label: string;
  }> = [
    { id: 'monochrome', label: 'Monochrome' },
    { id: 'antra', label: 'Antra' },
    { id: 'sockseek', label: 'Sockseek / Soulseek' },
  ];

  $: indexedFiles = libraryOverview?.total ?? 0;
  $: presentFiles = libraryOverview?.present ?? 0;
  $: spotifyItems =
    (sourceOverview?.likedSongs?.entryCount ?? 0) +
    savedAlbumTotal +
    (sourceOverview?.playlistCount ?? 0);
  $: availableAcquisitionProviders = acquisitionProviderOptions.filter(
    (option) => !acquisitionProviders.includes(option.id),
  );
  $: hasSockseekProvider = acquisitionProviders.includes('sockseek');
  $: hasAntraProvider = acquisitionProviders.includes('antra');

  function acquisitionProviderLabel(provider: AcquisitionProviderId): string {
    return (
      acquisitionProviderOptions.find((option) => option.id === provider)
        ?.label ?? provider
    );
  }

  function addAcquisitionProvider(provider: AcquisitionProviderId) {
    if (acquisitionProviders.includes(provider)) return;
    const providers = [...acquisitionProviders, provider];
    acquisitionProviders = providers;
    void onSaveAcquisition(acquisitionEnabled, providers);
  }

  function removeAcquisitionProvider(provider: AcquisitionProviderId) {
    if (acquisitionProviders.length <= 1) return;
    const providers = acquisitionProviders.filter(
      (candidate) => candidate !== provider,
    );
    acquisitionProviders = providers;
    void onSaveAcquisition(acquisitionEnabled, providers);
  }

  function moveAcquisitionProvider(
    provider: AcquisitionProviderId,
    direction: -1 | 1,
  ) {
    const currentIndex = acquisitionProviders.indexOf(provider);
    const nextIndex = currentIndex + direction;
    if (
      currentIndex < 0 ||
      nextIndex < 0 ||
      nextIndex >= acquisitionProviders.length
    )
      return;
    const next = [...acquisitionProviders];
    [next[currentIndex], next[nextIndex]] = [
      next[nextIndex],
      next[currentIndex],
    ];
    acquisitionProviders = next;
    void onSaveAcquisition(acquisitionEnabled, next);
  }

  function setAcquisitionEnabled(event: Event) {
    const target = event.currentTarget;
    if (!(target instanceof HTMLInputElement)) return;
    acquisitionEnabled = target.checked;
    void onSaveAcquisition(target.checked, acquisitionProviders);
  }

  function selectTab(tab: SettingsTab) {
    activeTab = tab;
    window.requestAnimationFrame(() => {
      const panel = document.querySelector('[data-settings-panel]');
      if (panel instanceof window.HTMLElement) panel.focus();
    });
  }

  function onTabKeydown(event: globalThis.KeyboardEvent, index: number) {
    let nextIndex: number;
    if (event.key === 'ArrowRight') nextIndex = (index + 1) % tabs.length;
    else if (event.key === 'ArrowLeft') {
      nextIndex = (index - 1 + tabs.length) % tabs.length;
    } else if (event.key === 'Home') nextIndex = 0;
    else if (event.key === 'End') nextIndex = tabs.length - 1;
    else return;

    event.preventDefault();
    const next = tabs[nextIndex];
    activeTab = next.id;
    window.requestAnimationFrame(() => {
      document.getElementById(`settings-tab-${next.id}`)?.focus();
    });
  }

  function formatDateTime(value: number | null | undefined) {
    if (!value) return 'Never';
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(new Date(value));
  }

  function syncLabel(run: SyncRun | null) {
    if (!run) return 'Not run yet';
    if (run.status === 'succeeded') return 'Completed';
    if (run.status === 'running') return 'Running';
    if (run.status === 'partial') return 'Completed with issues';
    if (run.status === 'cancelled') return 'Cancelled';
    return 'Failed';
  }

  function syncTone(run: SyncRun | null) {
    if (!run) return 'neutral';
    if (run.status === 'succeeded') return 'success';
    if (run.status === 'failed') return 'danger';
    if (run.status === 'partial') return 'warning';
    if (run.status === 'running') return 'primary';
    return 'neutral';
  }
</script>

<div class="settings-workspace">
  <header class="settings-heading">
    <div>
      <h1>Settings</h1>
      <p>
        Manage Refrain's connections, library, and synchronization behavior.
      </p>
    </div>
  </header>

  <div class="settings-tabs-shell">
    <div class="settings-tabs" role="tablist" aria-label="Settings categories">
      {#each tabs as tab, index (tab.id)}
        <button
          id={`settings-tab-${tab.id}`}
          type="button"
          role="tab"
          aria-selected={activeTab === tab.id}
          aria-controls={`settings-panel-${tab.id}`}
          tabindex={activeTab === tab.id ? 0 : -1}
          class:active={activeTab === tab.id}
          onclick={() => selectTab(tab.id)}
          onkeydown={(event) => onTabKeydown(event, index)}
        >
          <Icon name={tab.icon} size={16} />
          <span>{tab.label}</span>
        </button>
      {/each}
    </div>
  </div>

  <div class="settings-body">
    <div class="settings-scroll">
      {#if activeTab === 'general'}
        <div
          id="settings-panel-general"
          class="settings-panel"
          role="tabpanel"
          aria-labelledby="settings-tab-general"
          tabindex="-1"
          data-settings-panel
        >
          <div class="section-intro">
            <div>
              <h2>General</h2>
              <p>A quick view of the services and data Refrain is using.</p>
            </div>
            {#if appInfo}
              <span class="version-chip">v{appInfo.version}</span>
            {/if}
          </div>

          <div class="settings-overview-grid">
            <button
              class="overview-card"
              type="button"
              onclick={() => selectTab('spotify')}
            >
              <span class="overview-icon spotify"
                ><Icon name="spotify" size={18} /></span
              >
              <span class="overview-copy">
                <strong>Spotify</strong>
                <small
                  >{authStatus?.connected
                    ? 'Connected and ready'
                    : 'Not connected'}</small
                >
              </span>
              <span
                class={`status-dot ${authStatus?.connected ? 'success' : 'neutral'}`}
              ></span>
            </button>
            <button
              class="overview-card"
              type="button"
              onclick={() => selectTab('library')}
            >
              <span class="overview-icon"><Icon name="local" size={18} /></span>
              <span class="overview-copy">
                <strong>Local library</strong>
                <small
                  >{indexedFiles.toLocaleString()} indexed · {presentFiles.toLocaleString()}
                  present</small
                >
              </span>
              <Icon name="chevron-right" size={15} />
            </button>
            <button
              class="overview-card"
              type="button"
              onclick={() => selectTab('sync')}
            >
              <span class="overview-icon"
                ><Icon name="download" size={18} /></span
              >
              <span class="overview-copy">
                <strong>Acquisition</strong>
                <small
                  >{acquisitionEnabled
                    ? 'Enabled for missing tracks'
                    : 'Disabled'}</small
                >
              </span>
              <span
                class={`status-dot ${acquisitionEnabled ? 'primary' : 'neutral'}`}
              ></span>
            </button>
          </div>

          <article class="settings-card">
            <div class="card-heading">
              <span class="card-icon"><Icon name="activity" size={17} /></span>
              <div>
                <h3>Library health</h3>
                <p>Current local and Spotify source state.</p>
              </div>
            </div>
            <div class="stat-row">
              <div>
                <strong>{presentFiles.toLocaleString()}</strong><span
                  >Local files</span
                >
              </div>
              <div>
                <strong>{spotifyItems.toLocaleString()}</strong><span
                  >Spotify items</span
                >
              </div>
              <div>
                <strong>{libraryOverview?.missing ?? 0}</strong><span
                  >Missing files</span
                >
              </div>
              <div>
                <strong>{libraryOverview?.invalid ?? 0}</strong><span
                  >Invalid files</span
                >
              </div>
            </div>
          </article>

          <article class="settings-card">
            <div class="card-heading">
              <span class="card-icon"><Icon name="monitor" size={18} /></span>
              <div>
                <h3>Appearance</h3>
                <p>Choose how Refrain looks on this device.</p>
              </div>
            </div>

            <div class="theme-choice-grid" aria-label="Theme">
              <button
                type="button"
                aria-pressed={themePreference === 'system'}
                class:active={themePreference === 'system'}
                onclick={() => onChangeTheme('system')}
              >
                <span class="theme-preview system-preview">
                  <span></span><span></span>
                </span>
                <span class="theme-choice-label">
                  <Icon name="monitor" size={15} />
                  <span
                    ><strong>System</strong><small>Match your OS</small></span
                  >
                </span>
              </button>
              <button
                type="button"
                aria-pressed={themePreference === 'light'}
                class:active={themePreference === 'light'}
                onclick={() => onChangeTheme('light')}
              >
                <span class="theme-preview light-preview"></span>
                <span class="theme-choice-label">
                  <Icon name="sun" size={15} />
                  <span><strong>Light</strong><small>Always light</small></span>
                </span>
              </button>
              <button
                type="button"
                aria-pressed={themePreference === 'dark'}
                class:active={themePreference === 'dark'}
                onclick={() => onChangeTheme('dark')}
              >
                <span class="theme-preview dark-preview"></span>
                <span class="theme-choice-label">
                  <Icon name="moon" size={15} />
                  <span><strong>Dark</strong><small>Always dark</small></span>
                </span>
              </button>
            </div>
          </article>

          <article class="settings-card compact-card">
            <div class="card-heading">
              <span class="card-icon"><Icon name="info" size={17} /></span>
              <div>
                <h3>About Refrain</h3>
                <p>Application and local data location.</p>
              </div>
            </div>
            <dl class="detail-list">
              <div>
                <dt>Application</dt>
                <dd>
                  {appInfo ? `${appInfo.name} ${appInfo.version}` : 'Loading…'}
                </dd>
              </div>
              <div>
                <dt>Data directory</dt>
                <dd class="mono">{appInfo?.dataDir ?? 'Loading…'}</dd>
              </div>
            </dl>
          </article>
        </div>
      {:else if activeTab === 'sync'}
        <div
          id="settings-panel-sync"
          class="settings-panel"
          role="tabpanel"
          aria-labelledby="settings-tab-sync"
          tabindex="-1"
          data-settings-panel
        >
          <div class="section-intro">
            <div>
              <h2>Synchronization</h2>
              <p>
                Run the full local-first workflow or refresh Spotify source data
                independently.
              </p>
            </div>
            <button
              class="primary-button"
              type="button"
              onclick={onSyncLibrary}
              disabled={syncBusy || sourceBusy || authBusy}
            >
              <Icon name="sync" size={15} />
              {syncBusy
                ? syncScope === 'spotify'
                  ? 'Syncing Spotify…'
                  : 'Syncing local library…'
                : 'Sync Library'}
            </button>
          </div>

          {#if syncError}
            <div class="message error">
              <Icon name="warning" size={16} />{syncError}
            </div>
          {/if}

          <article class="settings-card sync-flow-card">
            <div class="card-heading">
              <span class="card-icon primary"
                ><Icon name="sync" size={17} /></span
              >
              <div>
                <h3>Full library synchronization</h3>
                <p>
                  Reconcile local files first, then materialize your tracked
                  Spotify selections.
                </p>
              </div>
            </div>
            <div class="sync-flow">
              <div class="sync-step">
                <span class="sync-step-index">1</span>
                <div>
                  <strong>Local</strong><small
                    >Scan, match, reconcile, and normalize existing files.</small
                  >
                </div>
              </div>
              <span class="sync-connector"></span>
              <div class="sync-step">
                <span class="sync-step-index">2</span>
                <div>
                  <strong>Spotify</strong><small
                    >Refresh tracked source state and queue missing material.</small
                  >
                </div>
              </div>
            </div>
          </article>

          <div class="two-column-cards">
            <article class="settings-card compact-card">
              <div class="card-heading inline-heading">
                <span class="card-icon"><Icon name="local" size={17} /></span>
                <div>
                  <h3>Local sync</h3>
                  <p>Latest reconciliation run.</p>
                </div>
                <span class={`status-pill ${syncTone(localSyncRun)}`}
                  >{syncLabel(localSyncRun)}</span
                >
              </div>
              <dl class="detail-list">
                <div>
                  <dt>Last run</dt>
                  <dd>{formatDateTime(localSyncRun?.startedAt)}</dd>
                </div>
                <div>
                  <dt>Matched</dt>
                  <dd>{localSyncRun?.matched ?? 0}</dd>
                </div>
                <div>
                  <dt>Needs review</dt>
                  <dd>{localSyncRun?.needsReview ?? 0}</dd>
                </div>
              </dl>
            </article>
            <article class="settings-card compact-card">
              <div class="card-heading inline-heading">
                <span class="card-icon"><Icon name="spotify" size={17} /></span>
                <div>
                  <h3>Spotify sync</h3>
                  <p>Latest tracked-source run.</p>
                </div>
                <span class={`status-pill ${syncTone(spotifySyncRun)}`}
                  >{syncLabel(spotifySyncRun)}</span
                >
              </div>
              <dl class="detail-list">
                <div>
                  <dt>Last run</dt>
                  <dd>{formatDateTime(spotifySyncRun?.startedAt)}</dd>
                </div>
                <div>
                  <dt>Missing</dt>
                  <dd>{spotifySyncRun?.missing ?? 0}</dd>
                </div>
                <div>
                  <dt>Acquisition failures</dt>
                  <dd>{spotifySyncRun?.acquisitionFailed ?? 0}</dd>
                </div>
              </dl>
            </article>
          </div>

          <article class="settings-card compact-card">
            <div class="card-heading inline-heading">
              <span class="card-icon"><Icon name="activity" size={17} /></span>
              <div>
                <h3>Recent synchronization</h3>
                <p>The latest Local and Spotify runs.</p>
              </div>
            </div>
            {#if recentSyncRuns.length > 0}
              <div
                class="sync-history"
                aria-label="Recent synchronization runs"
              >
                {#each recentSyncRuns as run (run.id)}
                  <div class="sync-history-row">
                    <div class="sync-history-main">
                      <strong
                        >{run.scope === 'spotify' ? 'Spotify' : 'Local'}</strong
                      >
                      <small
                        >{formatDateTime(
                          run.finishedAt ?? run.startedAt,
                        )}</small
                      >
                    </div>
                    <div class="sync-history-counts">
                      {#if run.scope === 'spotify'}
                        <span>+{run.sourceAdded} / −{run.sourceRemoved}</span>
                      {/if}
                      <span>{run.matched} matched</span>
                      <span>{run.missing} missing</span>
                      <span>{run.needsReview} review</span>
                      {#if run.acquisitionFailed > 0}
                        <span>{run.acquisitionFailed} failed</span>
                      {/if}
                    </div>
                    <span class={`status-pill ${syncTone(run)}`}
                      >{syncLabel(run)}</span
                    >
                  </div>
                {/each}
              </div>
            {:else}
              <p class="empty-inline">No synchronization runs yet.</p>
            {/if}
          </article>

          <article class="settings-card compact-card action-card">
            <div class="card-heading">
              <span class="card-icon"><Icon name="spotify" size={17} /></span>
              <div>
                <h3>Spotify source refresh</h3>
                <p>
                  Update liked songs, saved albums, and playlists without
                  running a full library sync.
                </p>
              </div>
            </div>
            <button
              class="secondary-button"
              type="button"
              onclick={onRefreshSpotify}
              disabled={sourceBusy || authBusy || !authStatus?.connected}
            >
              <Icon name="refresh" size={14} />
              {sourceBusy ? 'Refreshing…' : 'Refresh Spotify'}
            </button>
          </article>

          <article class="settings-card acquisition-card">
            <div class="card-heading acquisition-heading">
              <span class="card-icon"><Icon name="download" size={18} /></span>
              <div>
                <h3>Missing track acquisition</h3>
                <p>
                  Search enabled providers in priority order until Refrain finds
                  a strong enough match.
                </p>
              </div>
              <div class="pill-row">
                <span class="status-pill neutral"
                  >{acquisitionProviders.length} provider{acquisitionProviders.length ===
                  1
                    ? ''
                    : 's'}</span
                >
              </div>
            </div>

            <div class="provider-priority-list" aria-label="Provider priority">
              {#each acquisitionProviders as provider, index (provider)}
                <div class="provider-priority-row">
                  <div class="provider-priority-copy">
                    <span class="provider-priority-index">{index + 1}</span>
                    <div>
                      <strong>{acquisitionProviderLabel(provider)}</strong>
                      <small
                        >{index === 0
                          ? 'Highest priority'
                          : `Fallback ${index}`}</small
                      >
                    </div>
                  </div>
                  <div class="provider-priority-actions">
                    <button
                      class="text-button"
                      type="button"
                      aria-label={`Move ${acquisitionProviderLabel(provider)} up`}
                      onclick={() => moveAcquisitionProvider(provider, -1)}
                      disabled={acquisitionBusy || index === 0}>Up</button
                    >
                    <button
                      class="text-button"
                      type="button"
                      aria-label={`Move ${acquisitionProviderLabel(provider)} down`}
                      onclick={() => moveAcquisitionProvider(provider, 1)}
                      disabled={acquisitionBusy ||
                        index === acquisitionProviders.length - 1}>Down</button
                    >
                    <button
                      class="text-button danger"
                      type="button"
                      onclick={() => removeAcquisitionProvider(provider)}
                      disabled={acquisitionBusy ||
                        acquisitionProviders.length === 1}>Remove</button
                    >
                  </div>
                </div>
              {/each}
            </div>

            {#if availableAcquisitionProviders.length > 0}
              <div class="provider-add-row">
                <span>Add provider</span>
                {#each availableAcquisitionProviders as option (option.id)}
                  <button
                    class="secondary-button"
                    type="button"
                    onclick={() => addAcquisitionProvider(option.id)}
                    disabled={acquisitionBusy}>+ {option.label}</button
                  >
                {/each}
              </div>
            {/if}

            <label class="switch-row acquisition-switch">
              <span class="switch-copy"
                ><strong>Acquire missing tracks</strong><small
                  >Search during synchronization when a tracked Spotify item has
                  no local copy.</small
                ></span
              >
              <input
                type="checkbox"
                checked={acquisitionEnabled}
                onchange={setAcquisitionEnabled}
                disabled={acquisitionBusy}
              />
            </label>
            <div class="button-row">
              {#if acquisitionProviders.includes('monochrome')}
                <button
                  class="secondary-button"
                  type="button"
                  onclick={onCheckMonochrome}
                  disabled={acquisitionBusy}
                >
                  <Icon name="activity" size={14} />Check Monochrome
                </button>
              {/if}
              {#if hasAntraProvider}
                <button
                  class="secondary-button"
                  type="button"
                  onclick={onCheckAntra}
                  disabled={acquisitionBusy || !antraConfigured}
                >
                  <Icon name="activity" size={14} />Check Antra
                </button>
              {/if}
              {#if hasSockseekProvider}
                <button
                  class="secondary-button"
                  type="button"
                  onclick={onCheckSockseek}
                  disabled={acquisitionBusy || !soulseekConfigured}
                >
                  <Icon name="activity" size={14} />Check Sockseek
                </button>
              {/if}
            </div>
            {#if acquisitionProviders.includes('monochrome') && monochromeHealth}
              <div
                class={`message ${monochromeHealth.available ? 'success' : 'warning'}`}
              >
                <Icon
                  name={monochromeHealth.available ? 'check' : 'warning'}
                  size={15}
                />
                Monochrome · {monochromeHealth.message ?? 'Provider ready'}
                {#if monochromeHealthCached}
                  <span class="cached-status"
                    >Saved from the previous session</span
                  >
                {/if}
              </div>
            {/if}
            {#if hasAntraProvider && antraHealth}
              <div
                class={`message ${antraHealth.available ? 'success' : 'warning'}`}
              >
                <Icon
                  name={antraHealth.available ? 'check' : 'warning'}
                  size={15}
                />
                Antra · {antraHealth.message ?? 'Provider ready'}
                {#if antraHealthCached}
                  <span class="cached-status"
                    >Saved from the previous session</span
                  >
                {/if}
              </div>
            {/if}
            {#if hasSockseekProvider && sockseekHealth}
              <div
                class={`message ${sockseekHealth.available ? 'success' : 'warning'}`}
              >
                <Icon
                  name={sockseekHealth.available ? 'check' : 'warning'}
                  size={15}
                />
                Sockseek {sockseekHealth.version ?? 'unknown'} · {sockseekHealth.message ??
                  'Provider ready'}
                {#if sockseekHealthCached}
                  <span class="cached-status"
                    >Saved from the previous session</span
                  >
                {/if}
              </div>
            {/if}
            {#if acquisitionError}
              <div class="message error">
                <Icon name="warning" size={16} />{acquisitionError}
              </div>
            {/if}
          </article>

          {#if hasAntraProvider}
            <article class="settings-card">
              <div class="card-heading">
                <span class="card-icon"><Icon name="download" size={18} /></span
                >
                <div>
                  <h3>Antra account</h3>
                  <p>
                    Refrain uses Antra's device sign-in and stores the device
                    token in the operating system credential store.
                  </p>
                </div>
                <div class="pill-row">
                  <span
                    class={`status-pill ${antraConfigured ? 'success' : 'neutral'}`}
                    >{antraConfigured
                      ? 'Signed in'
                      : antraLoginPending
                        ? 'Waiting for approval'
                        : 'Sign-in required'}</span
                  >
                </div>
              </div>

              {#if antraLoginPending}
                <div class="technical-field">
                  <span>Device code</span>
                  <code>{antraUserCode ?? 'Waiting for code'}</code>
                </div>
                <div class="button-row">
                  <button
                    class="primary-button"
                    type="button"
                    onclick={onOpenAntraLogin}
                    disabled={acquisitionBusy || !antraVerificationUrl}
                    >Open sign-in</button
                  >
                  <button
                    class="text-button"
                    type="button"
                    onclick={onCancelAntra}
                    disabled={acquisitionBusy}>Cancel</button
                  >
                </div>
              {:else if antraConfigured}
                <div class="button-row">
                  <button
                    class="text-button danger"
                    type="button"
                    onclick={onClearAntra}
                    disabled={acquisitionBusy}>Sign out</button
                  >
                </div>
              {:else}
                <div class="button-row">
                  <button
                    class="primary-button"
                    type="button"
                    onclick={onStartAntra}
                    disabled={acquisitionBusy}>Sign in with Antra</button
                  >
                </div>
              {/if}
            </article>
          {/if}

          {#if hasSockseekProvider}
            <article class="settings-card">
              <div class="card-heading">
                <span class="card-icon"><Icon name="download" size={18} /></span
                >
                <div>
                  <h3>Soulseek account</h3>
                  <p>
                    Sockseek uses your Soulseek account. Refrain stores these
                    credentials in the operating system credential store.
                  </p>
                </div>
                <div class="pill-row">
                  <span
                    class={`status-pill ${soulseekConfigured ? 'success' : 'neutral'}`}
                    >{soulseekConfigured
                      ? 'Account saved'
                      : 'Account required'}</span
                  >
                </div>
              </div>
              <div class="two-fields">
                <div class="field-stack">
                  <label for="soulseek-username">Username</label>
                  <input
                    id="soulseek-username"
                    bind:value={soulseekUsername}
                    disabled={acquisitionBusy}
                    autocomplete="username"
                    spellcheck="false"
                  />
                </div>
                <div class="field-stack">
                  <label for="soulseek-password">Password</label>
                  <input
                    id="soulseek-password"
                    type="password"
                    bind:value={soulseekPassword}
                    disabled={acquisitionBusy}
                    autocomplete="current-password"
                    placeholder={soulseekConfigured
                      ? 'Saved in credential store'
                      : ''}
                  />
                </div>
              </div>
              <div class="button-row">
                <button
                  class="primary-button"
                  type="button"
                  onclick={onSaveSoulseek}
                  disabled={acquisitionBusy ||
                    !soulseekUsername.trim() ||
                    !soulseekPassword}
                >
                  Save credentials
                </button>
                {#if soulseekConfigured}
                  <button
                    class="text-button danger"
                    type="button"
                    onclick={onClearSoulseek}
                    disabled={acquisitionBusy}>Clear credentials</button
                  >
                {/if}
              </div>
            </article>
          {/if}

          <div class="setup-note">
            <span class="card-icon"><Icon name="info" size={16} /></span>
            <div>
              <strong>How acquisition works</strong>
              <p>
                Downloads stay in Refrain's staging area. A provider download is
                not treated as a library file until the later verification and
                import stage accepts it.
              </p>
            </div>
          </div>
        </div>
      {:else if activeTab === 'spotify'}
        <div
          id="settings-panel-spotify"
          class="settings-panel"
          role="tabpanel"
          aria-labelledby="settings-tab-spotify"
          tabindex="-1"
          data-settings-panel
        >
          <div class="section-intro">
            <div>
              <h2>Spotify</h2>
              <p>
                Connect your Spotify application and manage persisted source
                data.
              </p>
            </div>
            <span
              class={`status-pill ${authStatus?.connected ? 'success' : 'neutral'}`}
            >
              {authStatus?.connected ? 'Connected' : 'Not connected'}
            </span>
          </div>

          <article class="settings-card">
            <div class="card-heading">
              <span class="card-icon spotify"
                ><Icon name="spotify" size={18} /></span
              >
              <div>
                <h3>Spotify account</h3>
                <p>
                  The Client ID stays on this device. Refresh credentials are
                  stored in the operating system credential store.
                </p>
              </div>
            </div>

            <div class="field-stack">
              <label for="spotify-client-id">Spotify Client ID</label>
              <input
                id="spotify-client-id"
                bind:value={clientId}
                disabled={authBusy || sourceBusy || authStatus?.connected}
                autocomplete="off"
                spellcheck="false"
                placeholder="Paste your Spotify Client ID"
              />
            </div>

            <div class="technical-field">
              <span>Redirect URI</span>
              <code
                >{authStatus?.registeredRedirectUri ??
                  fallbackRedirectUri}</code
              >
            </div>

            {#if authError}
              <div class="message error">
                <Icon name="warning" size={16} />{authError}
              </div>
            {/if}
            {#if sourceError}
              <div class="message error">
                <Icon name="warning" size={16} />{sourceError}
              </div>
            {/if}

            <div class="button-row">
              {#if authStatus?.connected}
                <button
                  class="secondary-button"
                  type="button"
                  onclick={onDisconnectSpotify}
                  disabled={authBusy || sourceBusy}
                >
                  {authBusy ? 'Disconnecting…' : 'Disconnect'}
                </button>
                <button
                  class="primary-button"
                  type="button"
                  onclick={onRefreshSpotify}
                  disabled={authBusy || sourceBusy}
                >
                  <Icon name="refresh" size={14} />{sourceBusy
                    ? 'Refreshing…'
                    : 'Refresh Spotify'}
                </button>
              {:else}
                <button
                  class="primary-button"
                  type="button"
                  onclick={onConnectSpotify}
                  disabled={authBusy || !clientId.trim()}
                >
                  <Icon name="spotify" size={14} />{authBusy
                    ? 'Waiting for Spotify…'
                    : 'Connect Spotify'}
                </button>
              {/if}
            </div>
          </article>

          <article class="settings-card compact-card">
            <div class="card-heading inline-heading">
              <span class="card-icon"><Icon name="database" size={17} /></span>
              <div>
                <h3>Persisted source</h3>
                <p>What Refrain currently has stored from Spotify.</p>
              </div>
              {#if sourceOverview?.account?.lastSourceSyncAt}
                <span class="subtle-meta"
                  >Updated {formatDateTime(
                    sourceOverview.account.lastSourceSyncAt,
                  )}</span
                >
              {/if}
            </div>
            <div class="stat-row three">
              <div>
                <strong>{sourceOverview?.likedSongs?.entryCount ?? 0}</strong
                ><span>Liked Songs</span>
              </div>
              <div>
                <strong>{savedAlbumTotal}</strong><span>Saved albums</span>
              </div>
              <div>
                <strong>{sourceOverview?.playlistCount ?? 0}</strong><span
                  >Playlists</span
                >
              </div>
            </div>
            {#if sourceSummary}
              <div class="message success">
                <Icon name="check" size={15} />
                Refreshed {sourceSummary.refreshedPlaylists} playlists; {sourceSummary.unchangedPlaylists}
                unchanged.
              </div>
            {/if}
          </article>
        </div>
      {:else if activeTab === 'library'}
        <div
          id="settings-panel-library"
          class="settings-panel"
          role="tabpanel"
          aria-labelledby="settings-tab-library"
          tabindex="-1"
          data-settings-panel
        >
          <div class="section-intro">
            <div>
              <h2>Local library</h2>
              <p>
                Choose the folder Refrain observes and keep its index current.
              </p>
            </div>
            <span class="status-pill neutral"
              >{indexedFiles.toLocaleString()} indexed</span
            >
          </div>

          <article class="settings-card">
            <div class="card-heading">
              <span class="card-icon"><Icon name="folder" size={18} /></span>
              <div>
                <h3>Music folder</h3>
                <p>
                  Scanning reads metadata and file state. It does not move or
                  delete files.
                </p>
              </div>
            </div>
            <div class="field-stack">
              <label for="library-root">Library root</label>
              <div class="path-picker">
                <input
                  id="library-root"
                  bind:value={libraryRoot}
                  disabled={libraryBusy}
                  readonly
                  autocomplete="off"
                  spellcheck="false"
                  placeholder="Choose a music folder"
                  onclick={onPickLibraryRoot}
                  onkeydown={(event) => {
                    if (event.key === 'Enter' || event.key === ' ') {
                      event.preventDefault();
                      void onPickLibraryRoot();
                    }
                  }}
                />
                <button
                  class="secondary-button"
                  type="button"
                  onclick={onPickLibraryRoot}
                  disabled={libraryBusy}
                >
                  <Icon name="folder" size={14} />Choose
                </button>
              </div>
            </div>
            <div class="button-row">
              <button
                class="secondary-button"
                type="button"
                onclick={onSaveLibraryRoot}
                disabled={libraryBusy}>Save path</button
              >
              <button
                class="primary-button"
                type="button"
                onclick={onScanLibrary}
                disabled={libraryBusy || !libraryRoot.trim()}
              >
                <span class="scan-button-icon" class:is-spinning={libraryBusy}>
                  <Icon name="refresh" size={14} />
                </span>{libraryBusy ? 'Scanning…' : 'Scan Files'}
              </button>
            </div>
            {#if libraryProgress}
              <div class="message info">
                <Icon name="activity" size={15} />{libraryProgress.message}
              </div>
            {/if}
            {#if libraryError}
              <div class="message error">
                <Icon name="warning" size={16} />{libraryError}
              </div>
            {/if}
          </article>

          <article class="settings-card compact-card">
            <div class="card-heading">
              <span class="card-icon"><Icon name="database" size={17} /></span>
              <div>
                <h3>Index state</h3>
                <p>Current local file inventory.</p>
              </div>
            </div>
            <div class="stat-row">
              <div>
                <strong>{libraryOverview?.present ?? 0}</strong><span
                  >Present</span
                >
              </div>
              <div>
                <strong>{libraryOverview?.missing ?? 0}</strong><span
                  >Missing</span
                >
              </div>
              <div>
                <strong>{libraryOverview?.invalid ?? 0}</strong><span
                  >Invalid</span
                >
              </div>
              <div>
                <strong>{libraryOverview?.total ?? 0}</strong><span>Total</span>
              </div>
            </div>
            {#if librarySummary}
              <div class="scan-summary">
                Last scan: {librarySummary.added} added · {librarySummary.updated}
                updated · {librarySummary.moved} moved · {librarySummary.unchanged}
                unchanged
              </div>
            {/if}
          </article>
        </div>
      {:else}
        <div
          id="settings-panel-advanced"
          class="settings-panel"
          role="tabpanel"
          aria-labelledby="settings-tab-advanced"
          tabindex="-1"
          data-settings-panel
        >
          <div class="section-intro">
            <div>
              <h2>Advanced</h2>
              <p>
                Runtime details useful for setup, troubleshooting, and
                development.
              </p>
            </div>
          </div>

          <article class="settings-card compact-card">
            <div class="card-heading">
              <span class="card-icon"><Icon name="info" size={17} /></span>
              <div>
                <h3>Application runtime</h3>
                <p>Local installation details.</p>
              </div>
            </div>
            <dl class="detail-list">
              <div>
                <dt>Application</dt>
                <dd>
                  {appInfo ? `${appInfo.name} ${appInfo.version}` : 'Loading…'}
                </dd>
              </div>
              <div>
                <dt>Data directory</dt>
                <dd class="mono">{appInfo?.dataDir ?? 'Loading…'}</dd>
              </div>
              <div>
                <dt>Spotify redirect URI</dt>
                <dd class="mono">
                  {authStatus?.registeredRedirectUri ?? fallbackRedirectUri}
                </dd>
              </div>
            </dl>
          </article>

          <article class="settings-card compact-card">
            <div class="card-heading">
              <span class="card-icon"><Icon name="database" size={17} /></span>
              <div>
                <h3>Persisted data</h3>
                <p>High-level counts from Refrain's local state.</p>
              </div>
            </div>
            <dl class="detail-list">
              <div>
                <dt>Local files</dt>
                <dd>{indexedFiles.toLocaleString()}</dd>
              </div>
              <div>
                <dt>Liked Songs</dt>
                <dd>{sourceOverview?.likedSongs?.entryCount ?? 0}</dd>
              </div>
              <div>
                <dt>Saved albums</dt>
                <dd>{savedAlbumTotal}</dd>
              </div>
              <div>
                <dt>Playlists</dt>
                <dd>{sourceOverview?.playlistCount ?? 0}</dd>
              </div>
            </dl>
          </article>

          <div class="setup-note">
            <span class="card-icon"><Icon name="database" size={16} /></span>
            <div>
              <strong>Local-first configuration</strong>
              <p>
                Refrain keeps settings and source state in its application data
                directory. Credentials remain in the operating system credential
                store.
              </p>
            </div>
          </div>
        </div>
      {/if}
    </div>

    <aside class="settings-summary-rail" aria-label="Settings summary">
      <article class="summary-card account-card">
        <div class="summary-card-heading">
          <span class="summary-icon spotify"
            ><Icon name="spotify" size={17} /></span
          >
          <div>
            <strong>Spotify account</strong>
            <small
              >{authStatus?.connected
                ? 'Connected and ready'
                : 'Not connected'}</small
            >
          </div>
        </div>

        {#if sourceOverview?.account}
          <div class="account-identity">
            {#if sourceOverview.account.imageUrl}
              <img src={sourceOverview.account.imageUrl} alt="" />
            {:else}
              <span class="account-avatar-fallback">
                {(sourceOverview.account.displayName ?? 'S')
                  .slice(0, 1)
                  .toUpperCase()}
              </span>
            {/if}
            <div>
              <strong
                >{sourceOverview.account.displayName ??
                  'Spotify account'}</strong
              >
              <small
                >{formatDateTime(
                  sourceOverview.account.lastSourceSyncAt,
                )}</small
              >
            </div>
          </div>
        {/if}

        <div class="summary-actions">
          {#if authStatus?.connected}
            <button
              class="summary-secondary"
              type="button"
              onclick={onDisconnectSpotify}
              disabled={authBusy || sourceBusy}
            >
              {authBusy ? 'Disconnecting…' : 'Disconnect'}
            </button>
            <button
              class="summary-icon-button"
              type="button"
              title="Refresh Spotify"
              aria-label="Refresh Spotify"
              onclick={onRefreshSpotify}
              disabled={authBusy || sourceBusy}
            >
              <Icon name="refresh" size={14} />
            </button>
          {:else}
            <button
              class="summary-secondary"
              type="button"
              onclick={() => selectTab('spotify')}
            >
              Configure Spotify
            </button>
          {/if}
        </div>
      </article>

      <article class="summary-card">
        <div class="summary-card-heading">
          <span class="summary-icon"><Icon name="sync" size={17} /></span>
          <div>
            <strong>Synchronization</strong>
            <small>Run and inspect library sync.</small>
          </div>
        </div>
        <div class="summary-links">
          <button type="button" onclick={() => selectTab('sync')}>
            <span><Icon name="sync" size={14} />Sync settings</span>
            <Icon name="chevron-right" size={14} />
          </button>
          <button type="button" onclick={() => selectTab('spotify')}>
            <span><Icon name="spotify" size={14} />Spotify source</span>
            <Icon name="chevron-right" size={14} />
          </button>
          <button type="button" onclick={() => selectTab('sync')}>
            <span><Icon name="download" size={14} />Acquisition</span>
            <Icon name="chevron-right" size={14} />
          </button>
        </div>
      </article>

      <article class="summary-card">
        <div class="summary-card-heading">
          <span class="summary-icon"><Icon name="folder" size={17} /></span>
          <div>
            <strong>Local library</strong>
            <small>{indexedFiles.toLocaleString()} indexed files.</small>
          </div>
        </div>
        <div class="summary-library-state">
          <div>
            <span>Present</span><strong>{presentFiles.toLocaleString()}</strong>
          </div>
          <div>
            <span>Missing</span><strong>{libraryOverview?.missing ?? 0}</strong>
          </div>
        </div>
        <button
          class="summary-link-button"
          type="button"
          onclick={() => selectTab('library')}
        >
          Manage library <Icon name="chevron-right" size={14} />
        </button>
      </article>
    </aside>
  </div>
</div>

<style>
  .settings-workspace {
    display: flex;
    height: 100%;
    min-height: 0;
    min-width: 0;
    flex-direction: column;
    overflow: hidden;
  }

  .settings-heading {
    display: flex;
    flex: 0 0 auto;
    align-items: flex-start;
    background: var(--bg-content);
    padding: 4px 0 16px;
  }

  .settings-heading h1 {
    margin: 0;
    font-size: clamp(24px, 2vw, 34px);
    font-weight: 760;
    letter-spacing: -0.035em;
    line-height: 1.08;
  }

  .settings-heading p:not(.settings-eyebrow) {
    margin: 6px 0 0;
    color: var(--text-secondary);
    font-size: 12px;
    line-height: 1.5;
  }

  .status-dot {
    width: 7px;
    height: 7px;
    flex: 0 0 7px;
    border-radius: 50%;
    background: var(--text-disabled);
  }

  .status-dot.success {
    background: var(--green-600);
  }

  .status-dot.primary {
    background: var(--blue-600);
  }

  .settings-tabs-shell {
    flex: 0 0 auto;
    overflow-x: auto;
    border-bottom: 1px solid var(--divider);
    background: var(--bg-content);
    padding-bottom: 14px;
    scrollbar-width: none;
  }

  .settings-tabs-shell::-webkit-scrollbar {
    display: none;
  }

  .settings-tabs {
    display: grid;
    min-width: 520px;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 6px;
  }

  .settings-tabs button {
    display: flex;
    height: 40px;
    min-width: 0;
    align-items: center;
    justify-content: center;
    gap: 8px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    padding: 0 12px;
    color: var(--text-secondary);
    font-size: 10.5px;
    font-weight: 650;
    white-space: nowrap;
    cursor: pointer;
    transition:
      color 140ms cubic-bezier(0.23, 1, 0.32, 1),
      background-color 140ms cubic-bezier(0.23, 1, 0.32, 1),
      transform 120ms cubic-bezier(0.23, 1, 0.32, 1);
  }

  .settings-tabs button:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .settings-tabs button:active {
    transform: scale(0.98);
  }

  .settings-tabs button.active {
    border-color: var(--blue-600);
    background: var(--bg-selected);
    color: var(--blue-700);
  }

  .settings-tabs button.active :global(svg) {
    color: var(--blue-600);
  }

  .settings-body {
    display: grid;
    min-height: 0;
    flex: 1;
    grid-template-columns: minmax(0, 1fr) 232px;
    gap: 14px;
    overflow: hidden;
    background: var(--bg-content);
  }

  .settings-scroll {
    min-height: 0;
    overflow-y: auto;
    background: var(--bg-content);
    padding: 18px 4px 32px 0;
  }

  .settings-panel {
    display: grid;
    max-width: 880px;
    gap: 12px;
    outline: none;
  }

  .settings-summary-rail {
    display: grid;
    min-height: 0;
    align-content: start;
    gap: 10px;
    overflow-y: auto;
    padding: 18px 2px 32px 0;
    background: var(--bg-content);
  }

  .summary-card {
    border: 1px solid var(--border-default);
    border-radius: var(--radius-card);
    background: var(--bg-elevated);
    padding: 13px;
    box-shadow: 0 1px 2px rgb(15 23 42 / 2%);
  }

  .summary-card-heading {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 9px;
  }

  .summary-card-heading > div {
    display: grid;
    min-width: 0;
    gap: 2px;
  }

  .summary-card-heading strong,
  .account-identity strong {
    overflow: hidden;
    font-size: 10.5px;
    font-weight: 700;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .summary-card-heading small,
  .account-identity small {
    overflow: hidden;
    color: var(--text-secondary);
    font-size: 9px;
    line-height: 1.35;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .summary-icon {
    display: inline-grid;
    width: 32px;
    height: 32px;
    flex: 0 0 32px;
    place-items: center;
    border-radius: 8px;
    background: var(--bg-subtle);
    color: var(--text-secondary);
  }

  .summary-icon.spotify {
    background: var(--green-100);
    color: var(--green-600);
  }

  .account-identity {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr);
    align-items: center;
    gap: 9px;
    border-top: 1px solid var(--border-subtle);
    margin-top: 12px;
    padding-top: 11px;
  }

  .account-identity img,
  .account-avatar-fallback {
    display: grid;
    width: 34px;
    height: 34px;
    place-items: center;
    border-radius: 50%;
    object-fit: cover;
    box-shadow: inset 0 0 0 1px rgb(15 23 42 / 8%);
  }

  .account-avatar-fallback {
    background: var(--blue-100);
    color: var(--blue-700);
    font-size: 11px;
    font-weight: 750;
  }

  .account-identity > div {
    display: grid;
    min-width: 0;
    gap: 2px;
  }

  .summary-actions {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 7px;
    margin-top: 11px;
  }

  .summary-actions:has(.summary-secondary:only-child) {
    grid-template-columns: 1fr;
  }

  .summary-secondary,
  .summary-icon-button {
    border: 1px solid var(--border-default);
    border-radius: 7px;
    background: var(--bg-control);
    color: var(--text-primary);
    font-size: 9px;
    font-weight: 650;
    cursor: pointer;
  }

  .summary-link-button,
  .summary-links button {
    border: 0;
    background: transparent;
    color: var(--text-primary);
    font-size: 9px;
    font-weight: 650;
    cursor: pointer;
  }

  .summary-secondary {
    min-height: 30px;
    padding: 0 9px;
  }

  .summary-icon-button {
    display: grid;
    width: 30px;
    height: 30px;
    place-items: center;
    padding: 0;
  }

  .summary-secondary:hover:not(:disabled),
  .summary-icon-button:hover:not(:disabled),
  .summary-link-button:hover,
  .summary-links button:hover {
    background: var(--bg-hover);
  }

  .summary-secondary:disabled,
  .summary-icon-button:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .summary-links {
    display: grid;
    border-top: 1px solid var(--border-subtle);
    margin-top: 11px;
  }

  .summary-links button {
    display: flex;
    min-height: 40px;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    border: 0;
    border-top: 1px solid var(--border-subtle);
    border-radius: 0;
    padding: 0 2px;
    text-align: left;
  }

  .summary-links button:first-child {
    border-top: 0;
  }

  .summary-links button > span {
    display: inline-flex;
    min-width: 0;
    align-items: center;
    gap: 7px;
  }

  .summary-links button > :global(svg) {
    color: var(--text-tertiary);
  }

  .summary-library-state {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    border-top: 1px solid var(--border-subtle);
    margin-top: 11px;
    padding-top: 10px;
  }

  .summary-library-state > div {
    display: grid;
    gap: 2px;
  }

  .summary-library-state span {
    color: var(--text-tertiary);
    font-size: 8.5px;
  }

  .summary-library-state strong {
    font-size: 13px;
    font-variant-numeric: tabular-nums;
    font-weight: 720;
  }

  .summary-link-button {
    display: flex;
    width: 100%;
    min-height: 30px;
    align-items: center;
    justify-content: space-between;
    border-top: 1px solid var(--border-subtle);
    border-radius: 0;
    margin-top: 10px;
    padding: 9px 0 0;
    text-align: left;
  }

  .section-intro {
    display: flex;
    min-height: 46px;
    align-items: flex-start;
    justify-content: space-between;
    gap: 20px;
    padding: 0 1px 3px;
  }

  .section-intro h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 720;
    letter-spacing: -0.018em;
  }

  .section-intro p,
  .card-heading p,
  .switch-copy small,
  .setup-note p {
    margin: 3px 0 0;
    color: var(--text-secondary);
    font-size: 10.5px;
    line-height: 1.5;
  }

  .version-chip,
  .status-pill {
    display: inline-flex;
    min-height: 24px;
    align-items: center;
    border-radius: 999px;
    background: var(--bg-subtle);
    padding: 0 9px;
    color: var(--text-secondary);
    font-size: 9px;
    font-weight: 700;
    white-space: nowrap;
  }

  .status-pill.success {
    background: var(--green-100);
    color: var(--green-600);
  }

  .status-pill.warning {
    background: var(--orange-100);
    color: #b75a00;
  }

  .status-pill.danger {
    background: var(--red-100);
    color: var(--red-600);
  }

  .status-pill.primary {
    background: var(--blue-100);
    color: var(--blue-700);
  }

  .pill-row {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 6px;
  }

  .settings-overview-grid,
  .two-column-cards {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }

  .two-column-cards {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .overview-card,
  .settings-card,
  .setup-note {
    border: 1px solid var(--border-default);
    border-radius: var(--radius-card);
    background: var(--bg-elevated);
    box-shadow: 0 1px 2px rgb(15 23 42 / 2%);
  }

  .overview-card {
    display: grid;
    min-width: 0;
    min-height: 72px;
    grid-template-columns: 34px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 12px;
    text-align: left;
    cursor: pointer;
    transition:
      border-color 150ms cubic-bezier(0.23, 1, 0.32, 1),
      box-shadow 150ms cubic-bezier(0.23, 1, 0.32, 1),
      transform 120ms cubic-bezier(0.23, 1, 0.32, 1);
  }

  .overview-card:hover {
    border-color: var(--border-strong);
    box-shadow: 0 2px 7px rgb(15 23 42 / 5%);
  }

  .overview-card:active,
  .primary-button:active,
  .secondary-button:active {
    transform: scale(0.98);
  }

  .overview-card > :global(svg) {
    color: var(--text-tertiary);
  }

  .overview-icon,
  .card-icon {
    display: inline-grid;
    width: 34px;
    height: 34px;
    flex: 0 0 34px;
    place-items: center;
    border-radius: 8px;
    background: var(--bg-subtle);
    color: var(--text-secondary);
  }

  .overview-icon.spotify,
  .card-icon.spotify {
    background: var(--green-100);
    color: var(--green-600);
  }

  .card-icon.primary {
    background: var(--blue-100);
    color: var(--blue-700);
  }

  .overview-copy {
    display: grid;
    min-width: 0;
    gap: 3px;
  }

  .overview-copy strong,
  .switch-copy strong,
  .setup-note strong {
    font-size: 11px;
    font-weight: 680;
  }

  .overview-copy small {
    overflow: hidden;
    color: var(--text-secondary);
    font-size: 9.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .settings-card {
    padding: 16px;
  }

  .compact-card {
    padding: 14px 16px;
  }

  .card-heading {
    display: flex;
    min-width: 0;
    align-items: flex-start;
    gap: 11px;
  }

  .card-heading > div {
    min-width: 0;
    flex: 1;
  }

  .card-heading h3 {
    margin: 1px 0 0;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }

  .inline-heading {
    align-items: center;
  }

  .subtle-meta {
    margin-left: auto;
    color: var(--text-tertiary);
    font-size: 9px;
    white-space: nowrap;
  }

  .stat-row {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 0;
    border-top: 1px solid var(--border-subtle);
    margin-top: 14px;
    padding-top: 13px;
  }

  .stat-row.three {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .stat-row > div {
    display: grid;
    gap: 2px;
    border-left: 1px solid var(--border-subtle);
    padding: 0 12px;
  }

  .stat-row > div:first-child {
    border-left: 0;
    padding-left: 0;
  }

  .stat-row strong {
    font-size: 16px;
    font-variant-numeric: tabular-nums;
    font-weight: 720;
    letter-spacing: -0.02em;
  }

  .stat-row span {
    color: var(--text-secondary);
    font-size: 9px;
  }

  .detail-list {
    display: grid;
    gap: 0;
    margin: 12px 0 0;
  }

  .detail-list > div {
    display: grid;
    grid-template-columns: minmax(110px, 0.36fr) minmax(0, 1fr);
    gap: 16px;
    border-top: 1px solid var(--border-subtle);
    padding: 9px 0;
  }

  .detail-list > div:first-child {
    border-top: 0;
  }

  .detail-list dt {
    color: var(--text-secondary);
    font-size: 9.5px;
  }

  .detail-list dd {
    min-width: 0;
    margin: 0;
    font-size: 10px;
    font-variant-numeric: tabular-nums;
    font-weight: 580;
    text-align: right;
    overflow-wrap: anywhere;
  }

  .sync-history {
    display: grid;
    margin-top: 12px;
    border-top: 1px solid var(--border-subtle);
  }

  .sync-history-row {
    display: grid;
    grid-template-columns: minmax(110px, 0.55fr) minmax(0, 1.45fr) auto;
    gap: 14px;
    align-items: center;
    min-width: 0;
    padding: 10px 0;
    border-bottom: 1px solid var(--border-subtle);
  }

  .sync-history-row:last-child {
    border-bottom: 0;
  }

  .sync-history-main {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .sync-history-main strong {
    font-size: 10px;
    font-weight: 680;
  }

  .sync-history-main small,
  .sync-history-counts {
    color: var(--text-secondary);
    font-size: 9px;
  }

  .sync-history-counts {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 10px;
    min-width: 0;
    font-variant-numeric: tabular-nums;
  }

  .empty-inline {
    margin: 12px 0 0;
    color: var(--text-secondary);
    font-size: 10px;
  }

  .mono,
  code {
    font-family:
      ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 9px !important;
  }

  .field-stack {
    display: grid;
    gap: 6px;
    margin-top: 16px;
  }

  .field-stack label {
    color: var(--text-secondary);
    font-size: 9.5px;
    font-weight: 650;
  }

  .field-stack input {
    width: 100%;
    height: 36px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-control);
    background: var(--bg-control);
    padding: 0 11px;
    color: var(--text-primary);
    font-size: 10.5px;
    outline: none;
  }

  .field-stack input::placeholder {
    color: var(--text-tertiary);
  }

  .field-stack input:focus {
    border-color: var(--blue-600);
    box-shadow: 0 0 0 2px rgb(23 105 255 / 10%);
  }

  .field-stack input:disabled {
    background: var(--bg-subtle);
    color: var(--text-secondary);
  }

  .path-picker {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
  }

  .path-picker input {
    font-family:
      ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 9.5px;
    cursor: pointer;
  }

  .two-fields {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }

  .provider-priority-list {
    display: grid;
    gap: 7px;
  }

  .provider-priority-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-control);
    background: var(--bg-subtle);
    padding: 9px 10px;
  }

  .provider-priority-copy,
  .provider-priority-actions,
  .provider-add-row {
    display: flex;
    align-items: center;
  }

  .provider-priority-copy {
    gap: 9px;
    min-width: 0;
  }

  .provider-priority-copy > div {
    display: grid;
    gap: 2px;
  }

  .provider-priority-copy strong {
    color: var(--text-primary);
    font-size: 10.5px;
  }

  .provider-priority-copy small,
  .provider-add-row > span {
    color: var(--text-secondary);
    font-size: 9px;
  }

  .provider-priority-index {
    display: grid;
    width: 22px;
    height: 22px;
    place-items: center;
    border-radius: 999px;
    background: var(--bg-control);
    color: var(--text-secondary);
    font-size: 9px;
    font-weight: 700;
  }

  .provider-priority-actions {
    flex: 0 0 auto;
    gap: 6px;
  }

  .provider-add-row {
    flex-wrap: wrap;
    gap: 7px;
    margin-top: 8px;
  }

  .technical-field {
    display: grid;
    gap: 5px;
    border-radius: 8px;
    background: var(--bg-subtle);
    margin-top: 10px;
    padding: 10px 11px;
  }

  .technical-field span {
    color: var(--text-tertiary);
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }

  .technical-field code {
    color: var(--text-primary);
    overflow-wrap: anywhere;
  }

  .button-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 7px;
    margin-top: 14px;
  }

  .right-aligned {
    justify-content: flex-end;
  }

  .primary-button,
  .secondary-button,
  .text-button {
    display: inline-flex;
    min-height: 32px;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border-radius: 8px;
    padding: 0 11px;
    font-size: 9.5px;
    font-weight: 650;
    cursor: pointer;
    transition:
      background-color 140ms cubic-bezier(0.23, 1, 0.32, 1),
      border-color 140ms cubic-bezier(0.23, 1, 0.32, 1),
      transform 120ms cubic-bezier(0.23, 1, 0.32, 1);
  }

  .primary-button {
    border: 1px solid var(--blue-600);
    background: var(--blue-600);
    color: #fff;
  }

  .primary-button:hover:not(:disabled) {
    border-color: var(--blue-700);
    background: var(--blue-700);
  }

  .secondary-button {
    border: 1px solid var(--border-default);
    background: var(--bg-control);
    color: var(--text-primary);
  }

  .secondary-button:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  .text-button {
    border: 0;
    background: transparent;
    color: var(--text-secondary);
  }

  .text-button:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  .text-button.danger {
    color: var(--red-600);
  }

  .primary-button:disabled,
  .secondary-button:disabled,
  .text-button:disabled {
    cursor: not-allowed;
    opacity: 0.5;
    transform: none;
  }

  .message {
    display: flex;
    align-items: flex-start;
    gap: 7px;
    border-radius: 8px;
    margin-top: 12px;
    padding: 9px 10px;
    font-size: 9.5px;
    line-height: 1.45;
  }

  .message :global(svg) {
    margin-top: 1px;
    flex: 0 0 auto;
  }

  .message.error {
    background: var(--red-100);
    color: #b4232b;
  }

  .message.warning {
    background: var(--orange-100);
    color: #a95400;
  }

  .message.success {
    background: var(--green-100);
    color: #147d51;
  }

  .message.info {
    background: var(--blue-050);
    color: #315c9f;
  }

  .sync-flow {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 34px minmax(0, 1fr);
    align-items: center;
    gap: 8px;
    border-top: 1px solid var(--border-subtle);
    margin-top: 14px;
    padding-top: 14px;
  }

  .sync-step {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 10px;
  }

  .sync-step-index {
    display: grid;
    width: 24px;
    height: 24px;
    flex: 0 0 24px;
    place-items: center;
    border-radius: 50%;
    background: var(--blue-100);
    color: var(--blue-700);
    font-size: 9px;
    font-weight: 750;
  }

  .sync-step > div {
    display: grid;
    min-width: 0;
    gap: 2px;
  }

  .sync-step strong {
    font-size: 10.5px;
  }

  .sync-step small {
    color: var(--text-secondary);
    font-size: 9px;
    line-height: 1.4;
  }

  .sync-connector {
    height: 1px;
    background: linear-gradient(
      90deg,
      var(--border-default),
      #b8c8e8,
      var(--border-default)
    );
  }

  .action-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
  }

  .action-card .card-heading {
    flex: 1;
  }

  .acquisition-card {
    display: grid;
    gap: 14px;
  }

  .acquisition-heading {
    align-items: center;
  }

  .acquisition-heading .pill-row {
    margin-left: auto;
  }

  .acquisition-switch {
    border-top: 1px solid var(--border-subtle);
    padding-top: 13px;
  }

  .switch-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    cursor: pointer;
  }

  .switch-copy {
    display: grid;
    min-width: 0;
  }

  .switch-row input[type='checkbox'] {
    appearance: none;
    position: relative;
    width: 34px;
    height: 20px;
    flex: 0 0 34px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    background: var(--control-track);
    cursor: pointer;
    transition:
      background-color 150ms cubic-bezier(0.23, 1, 0.32, 1),
      border-color 150ms cubic-bezier(0.23, 1, 0.32, 1);
  }

  .switch-row input[type='checkbox']::after {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 2px rgb(15 23 42 / 18%);
    content: '';
    transition: transform 160ms cubic-bezier(0.23, 1, 0.32, 1);
  }

  .switch-row input[type='checkbox']:checked {
    border-color: var(--blue-600);
    background: var(--blue-600);
  }

  .switch-row input[type='checkbox']:checked::after {
    transform: translateX(14px);
  }

  .switch-row input[type='checkbox']:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  .setup-note {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    background: var(--bg-subtle);
    padding: 12px 13px;
  }

  .setup-note .card-icon {
    width: 30px;
    height: 30px;
    flex-basis: 30px;
    background: var(--bg-elevated);
  }

  .setup-note p {
    margin-top: 2px;
  }

  .scan-summary {
    border-top: 1px solid var(--border-subtle);
    margin-top: 12px;
    padding-top: 10px;
    color: var(--text-secondary);
    font-size: 9.5px;
  }

  .cached-status {
    margin-left: auto;
    color: currentColor;
    font-size: 8.5px;
    opacity: 0.72;
    white-space: nowrap;
  }

  .theme-choice-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
    margin-top: 16px;
  }

  .theme-choice-grid > button {
    display: grid;
    min-width: 0;
    gap: 9px;
    border: 1px solid var(--border-default);
    border-radius: 10px;
    background: var(--bg-control);
    padding: 8px;
    color: var(--text-primary);
    text-align: left;
    cursor: pointer;
    transition:
      border-color 140ms cubic-bezier(0.23, 1, 0.32, 1),
      box-shadow 140ms cubic-bezier(0.23, 1, 0.32, 1),
      transform 120ms cubic-bezier(0.23, 1, 0.32, 1);
  }

  .theme-choice-grid > button:hover {
    border-color: var(--border-strong);
  }

  .theme-choice-grid > button.active {
    border-color: var(--blue-600);
    box-shadow: 0 0 0 2px rgb(23 105 255 / 10%);
  }

  .theme-choice-grid > button:active {
    transform: scale(0.985);
  }

  .theme-preview {
    position: relative;
    display: block;
    height: 72px;
    overflow: hidden;
    border: 1px solid var(--border-default);
    border-radius: 7px;
  }

  .light-preview {
    background: linear-gradient(90deg, #eef1f5 0 23%, #ffffff 23% 100%);
  }

  .dark-preview {
    border-color: #343b48;
    background: linear-gradient(90deg, #11151c 0 23%, #191e27 23% 100%);
  }

  .system-preview {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }

  .system-preview > span:first-child {
    background: linear-gradient(90deg, #eef1f5 0 35%, #ffffff 35% 100%);
  }

  .system-preview > span:last-child {
    background: linear-gradient(90deg, #11151c 0 35%, #191e27 35% 100%);
  }

  .theme-choice-label {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 8px;
    padding: 0 2px 2px;
  }

  .theme-choice-label > span {
    display: grid;
    min-width: 0;
    gap: 1px;
  }

  .theme-choice-label strong {
    font-size: 10.5px;
    font-weight: 680;
  }

  .theme-choice-label small {
    color: var(--text-secondary);
    font-size: 9px;
  }

  :global(html[data-theme='dark']) .overview-card,
  :global(html[data-theme='dark']) .settings-card {
    background: var(--bg-elevated);
  }

  :global(html[data-theme='dark']) .setup-note {
    background: var(--bg-subtle);
  }

  :global(html[data-theme='dark']) .overview-icon,
  :global(html[data-theme='dark']) .card-icon {
    color: var(--text-secondary);
  }

  :global(html[data-theme='dark']) .field-stack label,
  :global(html[data-theme='dark']) .technical-field code,
  :global(html[data-theme='dark']) .secondary-button {
    color: var(--text-primary);
  }

  :global(html[data-theme='dark']) .field-stack input,
  :global(html[data-theme='dark']) .secondary-button {
    border-color: var(--border-default);
    background: var(--bg-control);
  }

  :global(html[data-theme='dark']) .field-stack input:disabled,
  :global(html[data-theme='dark']) .technical-field {
    background: var(--bg-subtle);
  }

  :global(html[data-theme='dark']) .switch-row input[type='checkbox'] {
    border-color: var(--border-strong);
    background: var(--control-track);
  }

  :global(html[data-theme='dark']) .switch-row input[type='checkbox']:checked {
    border-color: var(--blue-600);
    background: var(--blue-600);
  }

  :global(html[data-theme='dark']) .switch-row input[type='checkbox']::after {
    background: #f5f7fa;
  }

  :global(html[data-theme='dark']) .message.info {
    color: #9bbcff;
  }

  :global(html[data-theme='dark']) .message.success {
    color: #7ddcaf;
  }

  :global(html[data-theme='dark']) .message.warning {
    color: #ffba6a;
  }

  :global(html[data-theme='dark']) .message.error {
    color: #ff9da4;
  }

  @media (max-width: 1080px) {
    .settings-overview-grid {
      grid-template-columns: 1fr;
    }

    .overview-card {
      min-height: 62px;
    }

    .two-column-cards {
      grid-template-columns: 1fr;
    }

    .theme-choice-grid {
      grid-template-columns: 1fr;
    }
  }

  @media (max-width: 900px) {
    .settings-body {
      grid-template-columns: minmax(0, 1fr);
      background: var(--bg-content);
    }

    .settings-summary-rail {
      display: none;
    }

    .two-fields {
      grid-template-columns: 1fr;
    }

    .sync-history-row {
      grid-template-columns: minmax(0, 1fr) auto;
    }

    .sync-history-counts {
      grid-column: 1 / -1;
      grid-row: 2;
    }

    .stat-row {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      row-gap: 12px;
    }

    .stat-row > div:nth-child(3) {
      border-left: 0;
      padding-left: 0;
    }

    .stat-row.three {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }

    .stat-row.three > div:nth-child(3) {
      border-left: 1px solid var(--border-subtle);
      padding-left: 12px;
    }

    .sync-flow {
      grid-template-columns: 1fr;
    }

    .sync-connector {
      width: 1px;
      height: 16px;
      margin-left: 11px;
      background: var(--border-default);
    }

    .action-card {
      align-items: stretch;
      flex-direction: column;
    }

    .action-card > button {
      align-self: flex-start;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .settings-tabs button,
    .settings-tabs button::after,
    .overview-card,
    .primary-button,
    .secondary-button,
    .switch-row input[type='checkbox'],
    .switch-row input[type='checkbox']::after {
      transition: none;
    }
  }
</style>
