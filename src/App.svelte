<script lang="ts">
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount } from 'svelte';
  import Icon from './components/Icon.svelte';
  import IssuesView from './components/IssuesView.svelte';
  import LibraryView from './components/LibraryView.svelte';
  import LocalPlaylistsView from './components/LocalPlaylistsView.svelte';
  import SettingsView from './components/SettingsView.svelte';
  import SpotifyWorkspace from './components/SpotifyWorkspace.svelte';
  import StagingView from './components/StagingView.svelte';
  import { getAppInfo, type AppInfo } from './lib/app-info';
  import {
    cacheAntraProviderHealth,
    clearAntraDeviceToken,
    clearCachedAntraProviderHealth,
    getAntraAccountStatus,
    getAntraProviderHealth,
    getCachedAntraProviderHealth,
    openAntraVerificationUrl,
    pollAntraDeviceLogin,
    startAntraDeviceLogin,
    type AntraDeviceCode,
  } from './lib/antra';
  import { chooseLibraryRoot } from './lib/dialog';
  import {
    getMatchReview,
    listIssues,
    trashInvalidLocalFile,
    type IssueCounts,
    type IssueRow,
    type MatchReview,
  } from './lib/issues';
  import {
    getLocalLibraryOverview,
    listLibraryTracks,
    scanLocalLibrary,
    type LibraryTrackRow,
    type LocalLibraryOverview,
    type LocalLibraryScanProgress,
    type LocalLibraryScanSummary,
  } from './lib/library';
  import {
    clearMatchDecision,
    confirmMatch,
    rejectMatch,
  } from './lib/matching';
  import {
    loadPinnedSpotifyCollections,
    pinnedCollectionKey,
    savePinnedSpotifyCollections,
    togglePinnedSpotifyCollection,
  } from './lib/pinned-collections';
  import {
    addTracksToLocalPlaylist,
    createLocalPlaylist,
    deleteLocalPlaylist,
    getLocalPlaylist,
    listLocalPlaylists,
    moveLocalPlaylistEntry,
    removeLocalPlaylistEntry,
    renameLocalPlaylist,
    type LocalPlaylistDetail,
    type LocalPlaylistSummary,
  } from './lib/playlists';
  import {
    getSettings,
    updateSettings,
    type AcquisitionProviderId,
  } from './lib/settings';
  import {
    cacheMonochromeProviderHealth,
    clearCachedMonochromeProviderHealth,
    getCachedMonochromeProviderHealth,
    getMonochromeProviderHealth,
    type ProviderHealth,
  } from './lib/monochrome';
  import {
    cacheSockseekProviderHealth,
    clearCachedSockseekProviderHealth,
    clearSoulseekCredentials,
    getCachedSockseekProviderHealth,
    getSockseekProviderHealth,
    getSoulseekCredentialStatus,
    setSoulseekCredentials,
  } from './lib/sockseek';
  import {
    getThemePreference,
    setThemePreference,
    type ThemePreference,
  } from './lib/theme';
  import {
    getSourceCollectionPage,
    hydrateSpotifySource,
    listSpotifyPlaylists,
    listSpotifySavedAlbums,
    setSourceCollectionTracking,
    setSourceTrackTracking,
    setSourceTracksTracking,
    type SourceCollectionEntryView,
    type SourceCollectionSummary,
    type SpotifySourceOverview,
  } from './lib/source';
  import {
    cancelSpotifySourceRefresh,
    connectSpotify,
    disconnectSpotify,
    getSpotifyAuthStatus,
    refreshSpotifySource,
    spotifyErrorMessage,
    type SpotifyAuthStatus,
    type SpotifySourceRefreshProgress,
    type SpotifySourceRefreshSummary,
  } from './lib/spotify';
  import {
    cancelSync,
    listSyncRuns,
    startLocalSync,
    startSpotifySync,
    syncProgressEvent,
    type SyncProgress,
    type SyncRun,
  } from './lib/sync';
  import {
    cancelStagingTrack,
    continueStagingTracks,
    excludeStagingTrackFromTracking,
    listStagingItems,
    rejectStagingCandidate,
    resetAcquisitionSession,
    resolveStagingTrack,
    searchAgainStagingTrack,
    startStagingTrack,
    type StagingItem,
  } from './lib/staging';

  type AppView = 'library' | 'spotify' | 'staging' | 'issues' | 'settings';
  type LocalSection = 'songs' | 'playlists';
  type SpotifySection = 'liked' | 'albums' | 'playlists';
  type SidebarFeedback = {
    id: string;
    kind: 'error' | 'progress' | 'success';
    title: string;
    message: string;
    action?: 'cancelSync' | 'cancelSource';
  };

  const sourceProgressEvent = 'spotify-source-refresh-progress';
  const libraryProgressEvent = 'local-library-scan-progress';
  const collectionPageSize = 200;
  const collectionListPageSize = 100;
  const libraryTrackPageSize = 100;
  const issuePageSize = 100;
  const stagingPageSize = 500;

  let activeView: AppView = 'library';
  let localSection: LocalSection = 'songs';
  let localExpanded = true;
  let spotifyExpanded = true;
  let spotifySection: SpotifySection = 'liked';
  let appInfo: AppInfo | null = null;
  let authStatus: SpotifyAuthStatus | null = null;
  let clientId = '';
  let acquisitionEnabled = false;
  let acquisitionProviders: AcquisitionProviderId[] = ['monochrome'];
  let monochromeHealth: ProviderHealth | null =
    getCachedMonochromeProviderHealth();
  let monochromeHealthCached = monochromeHealth !== null;
  let antraConfigured = false;
  let antraLoginPending = false;
  let antraUserCode: string | null = null;
  let antraVerificationUrl: string | null = null;
  let antraLoginGeneration = 0;
  let antraHealth: ProviderHealth | null = getCachedAntraProviderHealth();
  let antraHealthCached = antraHealth !== null;
  let soulseekUsername = '';
  let soulseekPassword = '';
  let soulseekConfigured = false;
  let sockseekHealth: ProviderHealth | null = getCachedSockseekProviderHealth();
  let sockseekHealthCached = sockseekHealth !== null;
  let themePreference: ThemePreference = getThemePreference();
  let acquisitionBusy = false;
  let acquisitionError: string | null = null;
  let backendError: string | null = null;
  let authError: string | null = null;
  let authBusy = false;
  let sourceBusy = false;
  let sourceError: string | null = null;
  let sourceProgress: SpotifySourceRefreshProgress | null = null;
  let sourceSummary: SpotifySourceRefreshSummary | null = null;
  let syncBusy = false;
  let syncScope: 'local' | 'spotify' | null = null;
  let syncProgress: SyncProgress | null = null;
  let localSyncRun: SyncRun | null = null;
  let spotifySyncRun: SyncRun | null = null;
  let recentSyncRuns: SyncRun[] = [];
  let syncError: string | null = null;
  let trackingBusyId: number | null = null;
  let trackingBulkBusy = false;
  let sourceOverview: SpotifySourceOverview | null = null;
  let sourceHydrating = true;
  let sourceHydrationError: string | null = null;
  let loadedCollectionId: number | null = null;
  let savedAlbums: SourceCollectionSummary[] = [];
  let savedAlbumTotal = 0;
  let savedAlbumsLoadingMore = false;
  let selectedSavedAlbum: SourceCollectionSummary | null = null;
  let playlists: SourceCollectionSummary[] = [];
  let playlistTotal = 0;
  let playlistsLoadingMore = false;
  let selectedPlaylist: SourceCollectionSummary | null = null;
  let spotifyDetailsSidebarOpen = true;
  let pinnedSpotifyCollections = loadPinnedSpotifyCollections();
  let currentCollection: SourceCollectionSummary | null = null;
  let collectionEntries: SourceCollectionEntryView[] = [];
  let collectionTotal = 0;
  let collectionLoading = false;
  let collectionLoadingMore = false;
  let collectionError: string | null = null;
  let libraryRoot = '';
  let libraryBusy = false;
  let libraryError: string | null = null;
  let libraryProgress: LocalLibraryScanProgress | null = null;
  let librarySummary: LocalLibraryScanSummary | null = null;
  let libraryOverview: LocalLibraryOverview | null = null;
  let libraryTracks: LibraryTrackRow[] = [];
  let libraryTrackTotal = 0;
  let libraryTracksLoading = false;
  let libraryTracksLoadingMore = false;
  let libraryTracksError: string | null = null;
  let libraryTracksLoaded = false;
  let localPlaylists: LocalPlaylistSummary[] = [];
  let selectedLocalPlaylist: LocalPlaylistDetail | null = null;
  let localPlaylistLoading = false;
  let localPlaylistBusy = false;
  let localPlaylistError: string | null = null;
  let issues: IssueRow[] = [];
  let issueTotal = 0;
  let issueCounts: IssueCounts = {
    matchReview: 0,
    missingLocalFile: 0,
    localOnlyTrack: 0,
    inaccessibleCollection: 0,
    invalidLocalFile: 0,
    acquisitionFailed: 0,
  };
  let issuesLoading = false;
  let issuesLoadingMore = false;
  let issueError: string | null = null;
  let issuesLoaded = false;
  let selectedIssueId: string | null = null;
  let matchReview: MatchReview | null = null;
  let matchReviewLoading = false;
  let matchDecisionBusy = false;
  let issueTrashBusyId: number | null = null;
  let stagingItems: StagingItem[] = [];
  let stagingTotal = 0;
  let stagingLoading = false;
  let stagingBusy = false;
  let stagingError: string | null = null;
  let stagingLoaded = false;
  let selectedStagingTrackId: number | null = null;
  let selectedStagingTrackIds: number[] = [];
  let stagingSyncRun: SyncRun | null = null;
  let isMacOS = false;
  let sidebarFeedbackIndex = 0;
  let sidebarFeedbackDialog: SidebarFeedback | null = null;
  let sidebarFeedback: SidebarFeedback[] = [];
  let currentSidebarFeedback: SidebarFeedback | null = null;

  $: sidebarFeedback = [
    ...(syncError
      ? [
          {
            id: 'sync-error',
            kind: 'error' as const,
            title: 'Library sync failed',
            message: syncError,
          },
        ]
      : []),
    ...(sourceError
      ? [
          {
            id: 'source-error',
            kind: 'error' as const,
            title: 'Spotify error',
            message: sourceError,
          },
        ]
      : []),
    ...(libraryError
      ? [
          {
            id: 'library-error',
            kind: 'error' as const,
            title: 'Local library error',
            message: libraryError,
          },
        ]
      : []),
    ...(backendError
      ? [
          {
            id: 'backend-error',
            kind: 'error' as const,
            title: 'Application error',
            message: backendError,
          },
        ]
      : []),
    ...(syncBusy
      ? [
          {
            id: 'sync-progress',
            kind: 'progress' as const,
            title: 'Syncing library',
            message:
              syncProgress && syncProgress.scope === syncScope
                ? `${syncProgress.message}${
                    syncProgress.total !== null
                      ? ` · ${syncProgress.completed}/${syncProgress.total}`
                      : syncProgress.completed > 0
                        ? ` · ${syncProgress.completed}`
                        : ''
                  }`
                : syncScope === 'local'
                  ? 'Reconciling local files…'
                  : 'Syncing tracked music…',
            action: 'cancelSync' as const,
          },
        ]
      : []),
    ...(sourceBusy && sourceProgress
      ? [
          {
            id: 'source-progress',
            kind: 'progress' as const,
            title: 'Refreshing Spotify',
            message: `${sourceProgress.message}${
              sourceProgress.total !== null
                ? ` · ${sourceProgress.completed}/${sourceProgress.total}`
                : ''
            }`,
            action: 'cancelSource' as const,
          },
        ]
      : []),
    ...(libraryBusy && libraryProgress
      ? [
          {
            id: 'library-progress',
            kind: 'progress' as const,
            title: 'Scanning local library',
            message: `${libraryProgress.message}${
              libraryProgress.total !== null
                ? ` · ${libraryProgress.completed}/${libraryProgress.total}`
                : ` · ${libraryProgress.completed} scanned`
            }`,
          },
        ]
      : !libraryBusy && librarySummary
        ? [
            {
              id: 'library-summary',
              kind: 'success' as const,
              title: 'Library scan complete',
              message: `${librarySummary.discovered} files · ${librarySummary.added} added · ${librarySummary.updated} updated · ${librarySummary.moved} moved`,
            },
          ]
        : []),
  ];
  $: if (sidebarFeedbackIndex >= sidebarFeedback.length) {
    sidebarFeedbackIndex = Math.max(0, sidebarFeedback.length - 1);
  }
  $: currentSidebarFeedback = sidebarFeedback[sidebarFeedbackIndex] ?? null;

  $: stagingSyncRun = latestStagingSyncRun(localSyncRun, spotifySyncRun);

  onMount(() => {
    isMacOS = navigator.userAgent.includes('Macintosh');
    let disposed = false;
    let sourceUnlisten: UnlistenFn | undefined;
    let libraryUnlisten: UnlistenFn | undefined;
    let syncUnlisten: UnlistenFn | undefined;
    const stagingPoll = window.setInterval(() => {
      if (!disposed && activeView === 'staging') {
        void loadStaging(true);
      }
    }, 1000);

    const suppressWebviewContextMenu = (event: MouseEvent) => {
      const target = event.target;
      if (!(target instanceof Element)) {
        event.preventDefault();
        return;
      }

      const editable = target.closest(
        'input, textarea, [contenteditable="true"], [contenteditable=""]',
      );
      if (!editable) event.preventDefault();
    };

    window.addEventListener('contextmenu', suppressWebviewContextMenu);

    void listen<SpotifySourceRefreshProgress>(sourceProgressEvent, (event) => {
      sourceProgress = event.payload;
    }).then((stopListening) => {
      if (disposed) {
        stopListening();
      } else {
        sourceUnlisten = stopListening;
      }
    });
    void listen<LocalLibraryScanProgress>(libraryProgressEvent, (event) => {
      libraryProgress = event.payload;
    }).then((stopListening) => {
      if (disposed) {
        stopListening();
      } else {
        libraryUnlisten = stopListening;
      }
    });
    void listen<SyncProgress>(syncProgressEvent, (event) => {
      syncProgress = event.payload;
    }).then((stopListening) => {
      if (disposed) {
        stopListening();
      } else {
        syncUnlisten = stopListening;
      }
    });
    void initialize();

    return () => {
      disposed = true;
      sourceUnlisten?.();
      libraryUnlisten?.();
      syncUnlisten?.();
      window.clearInterval(stagingPoll);
      window.removeEventListener('contextmenu', suppressWebviewContextMenu);
    };
  });

  async function initialize() {
    try {
      const [info, status, settings, localOverview] = await Promise.all([
        getAppInfo(),
        getSpotifyAuthStatus(),
        getSettings(),
        getLocalLibraryOverview(),
      ]);
      appInfo = info;
      authStatus = status;
      clientId = status.clientId ?? '';
      libraryRoot = settings.libraryRoot ?? '';
      acquisitionEnabled = settings.acquisitionEnabled;
      acquisitionProviders = settings.acquisitionProviders;
      libraryOverview = localOverview;
      backendError = null;
      try {
        const soulseekStatus = await getSoulseekCredentialStatus();
        soulseekConfigured = soulseekStatus.configured;
        soulseekUsername = soulseekStatus.username ?? '';
      } catch (error) {
        acquisitionError = operationError(
          error,
          'Could not load Soulseek account status.',
        );
      }
      try {
        const antraStatus = await getAntraAccountStatus();
        antraConfigured = antraStatus.configured;
      } catch (error) {
        acquisitionError = operationError(
          error,
          'Could not load Antra account status.',
        );
      }
      await Promise.all([
        hydrateSourceState(),
        loadLibraryTracks(),
        loadLocalPlaylists(),
        loadIssues(),
        loadStaging(true),
      ]);
    } catch (error) {
      backendError = spotifyErrorMessage(error);
      sourceHydrating = false;
    }
  }

  async function hydrateSourceState(reloadActiveSection = true) {
    sourceHydrating = true;
    sourceHydrationError = null;
    try {
      const hydrated = await hydrateSpotifySource();
      sourceOverview = hydrated.overview;
      savedAlbums = hydrated.savedAlbums.items;
      savedAlbumTotal = hydrated.savedAlbums.total;
      playlists = hydrated.playlists.items;
      playlistTotal = hydrated.playlists.total;

      if (selectedSavedAlbum) {
        selectedSavedAlbum =
          savedAlbums.find((album) => album.id === selectedSavedAlbum?.id) ??
          null;
      }

      if (selectedPlaylist) {
        selectedPlaylist =
          playlists.find((playlist) => playlist.id === selectedPlaylist?.id) ??
          null;
      }

      if (reloadActiveSection && activeView === 'spotify') {
        await loadSpotifySection(spotifySection, true);
      }
    } catch (error) {
      sourceHydrationError = spotifyErrorMessage(error);
    } finally {
      sourceHydrating = false;
    }
  }

  async function connect() {
    authBusy = true;
    authError = null;
    try {
      authStatus = await connectSpotify(clientId);
      clientId = authStatus.clientId ?? clientId.trim();
    } catch (error) {
      authError = spotifyErrorMessage(error);
    } finally {
      authBusy = false;
    }
  }

  async function disconnect() {
    authBusy = true;
    authError = null;
    try {
      authStatus = await disconnectSpotify();
      sourceProgress = null;
      sourceSummary = null;
    } catch (error) {
      authError = spotifyErrorMessage(error);
    } finally {
      authBusy = false;
    }
  }

  async function refreshSource() {
    sourceBusy = true;
    sourceError = null;
    sourceSummary = null;
    sourceProgress = {
      phase: 'starting',
      completed: 0,
      total: null,
      message: 'Starting Spotify source refresh',
    };
    try {
      sourceSummary = await refreshSpotifySource();
      await hydrateSourceState();
      await Promise.all([
        loadLocalPlaylists(),
        loadIssues(),
        loadStaging(true),
      ]);
    } catch (error) {
      sourceError = spotifyErrorMessage(error);
    } finally {
      sourceBusy = false;
    }
  }

  async function cancelSourceRefresh() {
    try {
      await cancelSpotifySourceRefresh();
    } catch (error) {
      sourceError = spotifyErrorMessage(error);
    }
  }

  async function runLibrarySynchronization() {
    if (syncBusy || sourceBusy || authBusy || stagingBusy) return;

    syncBusy = true;
    syncScope = 'local';
    syncProgress = null;
    syncError = null;
    try {
      await resetAcquisitionSession();
      selectedStagingTrackIds = [];
      selectedStagingTrackId = null;
      await loadStaging(true);

      const localRun = await startLocalSync('manual');
      localSyncRun = localRun;
      if (localRun.status === 'failed') {
        syncError = localRun.errorMessage ?? 'Local reconciliation failed.';
        return;
      }
      if (localRun.status === 'cancelled') return;

      syncScope = 'spotify';
      syncProgress = null;
      const spotifyRun = await startSpotifySync('manual');
      spotifySyncRun = spotifyRun;
      if (spotifyRun.status === 'failed') {
        syncError =
          spotifyRun.errorMessage ?? 'Library synchronization failed.';
      }

      libraryOverview = await getLocalLibraryOverview();
      await Promise.all([
        hydrateSourceState(),
        loadLibraryTracks(),
        loadLocalPlaylists(),
        loadIssues(),
        loadStaging(true),
      ]);
    } catch (error) {
      syncError = operationError(error, 'Could not synchronize the library.');
    } finally {
      syncBusy = false;
      syncScope = null;
      syncProgress = null;
    }
  }

  async function cancelSynchronization() {
    try {
      await cancelSync();
    } catch (error) {
      syncError = operationError(error, 'Could not cancel synchronization.');
    }
  }

  async function persistLibraryRoot() {
    const current = await getSettings();
    const normalized = libraryRoot.trim();
    await updateSettings({ ...current, libraryRoot: normalized || null });
    libraryRoot = normalized;
  }

  async function saveLibraryRoot() {
    libraryBusy = true;
    libraryError = null;
    try {
      await persistLibraryRoot();
    } catch (error) {
      libraryError = spotifyErrorMessage(error);
    } finally {
      libraryBusy = false;
    }
  }

  async function saveAcquisitionSettings(
    enabled = acquisitionEnabled,
    providers = acquisitionProviders,
  ) {
    acquisitionBusy = true;
    acquisitionError = null;
    try {
      if (enabled && providers.includes('sockseek') && !soulseekConfigured) {
        throw new Error(
          'Save your Soulseek account credentials before enabling Sockseek acquisition.',
        );
      }
      const current = await getSettings();
      await updateSettings({
        ...current,
        acquisitionEnabled: enabled,
        acquisitionProviders: providers,
      });
      acquisitionEnabled = enabled;
      acquisitionProviders = providers;
    } catch (error) {
      acquisitionError = operationError(
        error,
        'Could not save acquisition settings.',
      );
      try {
        const persisted = await getSettings();
        acquisitionEnabled = persisted.acquisitionEnabled;
        acquisitionProviders = persisted.acquisitionProviders;
      } catch {
        // Keep the original acquisition error when persisted settings cannot be reloaded.
      }
    } finally {
      acquisitionBusy = false;
    }
  }

  async function checkMonochromeHealth() {
    acquisitionBusy = true;
    acquisitionError = null;
    monochromeHealth = null;
    monochromeHealthCached = false;
    clearCachedMonochromeProviderHealth();
    try {
      monochromeHealth = await getMonochromeProviderHealth();
      cacheMonochromeProviderHealth(monochromeHealth);
    } catch (error) {
      acquisitionError = operationError(
        error,
        'Could not check the Monochrome provider.',
      );
    } finally {
      acquisitionBusy = false;
    }
  }

  async function checkAntraHealth() {
    acquisitionBusy = true;
    acquisitionError = null;
    antraHealth = null;
    antraHealthCached = false;
    clearCachedAntraProviderHealth();
    try {
      antraHealth = await getAntraProviderHealth();
      cacheAntraProviderHealth(antraHealth);
    } catch (error) {
      acquisitionError = operationError(
        error,
        'Could not check the Antra provider.',
      );
    } finally {
      acquisitionBusy = false;
    }
  }

  function wait(milliseconds: number) {
    return new Promise<void>((resolve) =>
      window.setTimeout(resolve, milliseconds),
    );
  }

  async function runAntraLoginPolling(
    started: AntraDeviceCode,
    generation: number,
  ) {
    const deadline = Date.now() + Math.max(1, started.expiresIn) * 1_000;
    try {
      while (
        antraLoginPending &&
        generation === antraLoginGeneration &&
        Date.now() < deadline
      ) {
        await wait(Math.max(1, started.interval) * 1_000);
        if (!antraLoginPending || generation !== antraLoginGeneration) return;
        const status = await pollAntraDeviceLogin(started.deviceCode);
        if (status.status === 'approved' && status.configured) {
          antraConfigured = true;
          antraHealth = null;
          antraHealthCached = false;
          clearCachedAntraProviderHealth();
          antraLoginPending = false;
          antraUserCode = null;
          antraVerificationUrl = null;
          return;
        }
        if (status.status === 'error') {
          throw new Error(status.error ?? 'Antra sign-in failed.');
        }
      }
      if (generation === antraLoginGeneration && antraLoginPending) {
        throw new Error('Antra sign-in expired. Start the sign-in again.');
      }
    } catch (error) {
      if (generation !== antraLoginGeneration) return;
      acquisitionError = operationError(
        error,
        'Could not finish Antra sign-in.',
      );
      antraLoginPending = false;
      antraUserCode = null;
      antraVerificationUrl = null;
    }
  }

  async function startAntraLogin() {
    acquisitionBusy = true;
    acquisitionError = null;
    antraHealth = null;
    antraHealthCached = false;
    clearCachedAntraProviderHealth();
    try {
      const started = await startAntraDeviceLogin();
      const generation = ++antraLoginGeneration;
      antraLoginPending = true;
      antraUserCode = started.userCode;
      antraVerificationUrl = started.verificationUrl;
      try {
        await openAntraVerificationUrl(started.verificationUrl);
      } catch (error) {
        acquisitionError = operationError(
          error,
          'Could not open the Antra sign-in page. Use the Open sign-in button.',
        );
      }
      void runAntraLoginPolling(started, generation);
    } catch (error) {
      acquisitionError = operationError(
        error,
        'Could not start Antra sign-in.',
      );
      antraLoginPending = false;
      antraUserCode = null;
      antraVerificationUrl = null;
    } finally {
      acquisitionBusy = false;
    }
  }

  function cancelAntraLogin() {
    antraLoginGeneration += 1;
    antraLoginPending = false;
    antraUserCode = null;
    antraVerificationUrl = null;
  }

  async function openAntraLoginPage() {
    if (!antraVerificationUrl) return;
    try {
      await openAntraVerificationUrl(antraVerificationUrl);
    } catch (error) {
      acquisitionError = operationError(
        error,
        'Could not open the Antra sign-in page.',
      );
    }
  }

  async function clearAntraAccount() {
    acquisitionBusy = true;
    acquisitionError = null;
    cancelAntraLogin();
    antraHealth = null;
    antraHealthCached = false;
    clearCachedAntraProviderHealth();
    try {
      const status = await clearAntraDeviceToken();
      antraConfigured = status.configured;
    } catch (error) {
      acquisitionError = operationError(error, 'Could not sign out of Antra.');
    } finally {
      acquisitionBusy = false;
    }
  }

  async function saveSoulseekAccount() {
    acquisitionBusy = true;
    acquisitionError = null;
    sockseekHealth = null;
    sockseekHealthCached = false;
    clearCachedSockseekProviderHealth();
    try {
      const status = await setSoulseekCredentials(
        soulseekUsername.trim(),
        soulseekPassword,
      );
      soulseekConfigured = status.configured;
      soulseekUsername = status.username ?? '';
      soulseekPassword = '';
    } catch (error) {
      acquisitionError = operationError(
        error,
        'Could not save Soulseek credentials.',
      );
    } finally {
      acquisitionBusy = false;
    }
  }

  async function clearSoulseekAccount() {
    acquisitionBusy = true;
    acquisitionError = null;
    sockseekHealth = null;
    sockseekHealthCached = false;
    clearCachedSockseekProviderHealth();
    try {
      const status = await clearSoulseekCredentials();
      soulseekConfigured = status.configured;
      soulseekUsername = status.username ?? '';
      soulseekPassword = '';
      if (acquisitionProviders.includes('sockseek')) {
        const current = await getSettings();
        acquisitionProviders = acquisitionProviders.filter(
          (provider) => provider !== 'sockseek',
        );
        if (acquisitionProviders.length === 0) {
          acquisitionProviders = ['monochrome'];
        }
        await updateSettings({ ...current, acquisitionProviders });
      }
    } catch (error) {
      acquisitionError = operationError(
        error,
        'Could not clear Soulseek credentials.',
      );
    } finally {
      acquisitionBusy = false;
    }
  }

  async function checkSockseekHealth() {
    acquisitionBusy = true;
    acquisitionError = null;
    sockseekHealth = null;
    sockseekHealthCached = false;
    clearCachedSockseekProviderHealth();
    try {
      sockseekHealth = await getSockseekProviderHealth();
      cacheSockseekProviderHealth(sockseekHealth);
    } catch (error) {
      acquisitionError = operationError(
        error,
        'Could not check the Sockseek provider.',
      );
    } finally {
      acquisitionBusy = false;
    }
  }

  function changeTheme(preference: ThemePreference) {
    themePreference = preference;
    setThemePreference(preference);
  }

  async function scanLibraryRoot() {
    libraryBusy = true;
    libraryError = null;
    librarySummary = null;
    libraryProgress = {
      phase: 'starting',
      completed: 0,
      total: null,
      message: 'Preparing local library scan',
    };
    try {
      await persistLibraryRoot();
      librarySummary = await scanLocalLibrary();
      libraryOverview = await getLocalLibraryOverview();
      await Promise.all([
        loadLibraryTracks(),
        loadLocalPlaylists(),
        loadIssues(),
        loadStaging(true),
      ]);
    } catch (error) {
      libraryError = spotifyErrorMessage(error);
      libraryProgress = null;
    } finally {
      libraryBusy = false;
    }
  }

  async function pickLibraryRoot() {
    if (libraryBusy) return;
    libraryError = null;
    try {
      const selected = await chooseLibraryRoot(libraryRoot.trim() || undefined);
      if (selected) {
        libraryRoot = selected;
      }
    } catch (error) {
      libraryError = operationError(error, 'Could not open the folder picker.');
    }
  }

  function openView(view: AppView) {
    const wasActive = activeView === view;
    activeView = view;
    collectionError = null;

    if (wasActive) return;

    if (view === 'library') {
      if (!libraryTracksLoaded && !libraryTracksLoading) {
        void loadLibraryTracks();
      }
    } else if (view === 'spotify') {
      void loadSpotifySection(spotifySection);
    } else if (view === 'staging') {
      if (!stagingLoaded && !stagingLoading) void loadStaging();
    } else if (view === 'issues') {
      if (!issuesLoaded && !issuesLoading) void loadIssues();
    }
  }

  function openLocalSection(section: LocalSection) {
    localSection = section;
    localExpanded = true;
    activeView = 'library';
    collectionError = null;

    if (section === 'songs') {
      if (!libraryTracksLoaded && !libraryTracksLoading) {
        void loadLibraryTracks();
      }
    } else {
      selectedLocalPlaylist = null;
      void loadLocalPlaylists();
    }
  }

  function isSpotifySourceView(view: AppView): boolean {
    return view === 'spotify';
  }

  async function loadSpotifySection(section: SpotifySection, force = false) {
    spotifySection = section;
    collectionError = null;

    if (section === 'liked' && sourceOverview?.likedSongs) {
      if (
        force ||
        currentCollection?.id !== sourceOverview.likedSongs.id ||
        loadedCollectionId !== sourceOverview.likedSongs.id
      ) {
        await loadCollection(sourceOverview.likedSongs);
      }
    } else if (section === 'albums' && selectedSavedAlbum) {
      if (
        force ||
        currentCollection?.id !== selectedSavedAlbum.id ||
        loadedCollectionId !== selectedSavedAlbum.id
      ) {
        await loadCollection(selectedSavedAlbum);
      }
    } else if (section === 'playlists' && selectedPlaylist) {
      if (
        force ||
        currentCollection?.id !== selectedPlaylist.id ||
        loadedCollectionId !== selectedPlaylist.id
      ) {
        await loadCollection(selectedPlaylist);
      }
    } else {
      currentCollection = null;
      collectionEntries = [];
      collectionTotal = 0;
      loadedCollectionId = null;
    }
  }

  function togglePinnedCollection(collection: SourceCollectionSummary) {
    pinnedSpotifyCollections = togglePinnedSpotifyCollection(
      pinnedSpotifyCollections,
      collection,
    );
    savePinnedSpotifyCollections(pinnedSpotifyCollections);
  }

  function closeSpotifyCollection() {
    if (spotifySection === 'albums') selectedSavedAlbum = null;
    if (spotifySection === 'playlists') selectedPlaylist = null;
    currentCollection = null;
    collectionEntries = [];
    collectionTotal = 0;
    loadedCollectionId = null;
    collectionError = null;
  }

  async function loadLibraryTracks(append = false) {
    const offset = append ? libraryTracks.length : 0;
    if (append) {
      libraryTracksLoadingMore = true;
    } else {
      libraryTracksLoading = true;
      libraryTracksError = null;
    }
    try {
      const page = await listLibraryTracks(offset, libraryTrackPageSize);
      libraryTracks = append ? [...libraryTracks, ...page.items] : page.items;
      libraryTrackTotal = page.total;
      libraryTracksLoaded = true;
    } catch (error) {
      libraryTracksError = operationError(
        error,
        'Could not load the local library.',
      );
    } finally {
      libraryTracksLoading = false;
      libraryTracksLoadingMore = false;
    }
  }

  async function loadMoreLibraryTracks() {
    if (libraryTracksLoadingMore || libraryTracks.length >= libraryTrackTotal) {
      return;
    }
    await loadLibraryTracks(true);
  }

  async function refreshLocalPlaylistSummaries() {
    localPlaylists = await listLocalPlaylists();
  }

  async function loadLocalPlaylists(selectFirst = false) {
    localPlaylistLoading = true;
    localPlaylistError = null;
    try {
      await refreshLocalPlaylistSummaries();
      if (
        selectedLocalPlaylist &&
        !localPlaylists.some(
          (playlist) => playlist.id === selectedLocalPlaylist?.id,
        )
      ) {
        selectedLocalPlaylist = null;
      } else if (selectedLocalPlaylist) {
        selectedLocalPlaylist = await getLocalPlaylist(
          selectedLocalPlaylist.id,
        );
      }
      if (selectFirst && !selectedLocalPlaylist && localPlaylists.length > 0) {
        selectedLocalPlaylist = await getLocalPlaylist(localPlaylists[0].id);
      }
    } catch (error) {
      localPlaylistError = operationError(
        error,
        'Could not load local playlists.',
      );
    } finally {
      localPlaylistLoading = false;
    }
  }

  async function selectLocalPlaylist(playlistId: number) {
    if (selectedLocalPlaylist?.id === playlistId || localPlaylistBusy) return;
    localPlaylistLoading = true;
    localPlaylistError = null;
    try {
      selectedLocalPlaylist = await getLocalPlaylist(playlistId);
    } catch (error) {
      localPlaylistError = operationError(
        error,
        'Could not load the playlist.',
      );
    } finally {
      localPlaylistLoading = false;
    }
  }

  async function createNewLocalPlaylist(name: string) {
    localPlaylistBusy = true;
    localPlaylistError = null;
    try {
      selectedLocalPlaylist = await createLocalPlaylist(name);
      await refreshLocalPlaylistSummaries();
    } catch (error) {
      localPlaylistError = operationError(
        error,
        'Could not create the playlist.',
      );
    } finally {
      localPlaylistBusy = false;
    }
  }

  async function renameSelectedLocalPlaylist(name: string) {
    if (!selectedLocalPlaylist) return;
    localPlaylistBusy = true;
    localPlaylistError = null;
    try {
      selectedLocalPlaylist = await renameLocalPlaylist(
        selectedLocalPlaylist.id,
        name,
      );
      await refreshLocalPlaylistSummaries();
    } catch (error) {
      localPlaylistError = operationError(
        error,
        'Could not rename the playlist.',
      );
    } finally {
      localPlaylistBusy = false;
    }
  }

  async function deleteSelectedLocalPlaylist() {
    if (!selectedLocalPlaylist || localPlaylistBusy) return;
    if (!window.confirm(`Delete “${selectedLocalPlaylist.name}”?`)) return;
    localPlaylistBusy = true;
    localPlaylistError = null;
    try {
      await deleteLocalPlaylist(selectedLocalPlaylist.id);
      selectedLocalPlaylist = null;
      await refreshLocalPlaylistSummaries();
    } catch (error) {
      localPlaylistError = operationError(
        error,
        'Could not delete the playlist.',
      );
    } finally {
      localPlaylistBusy = false;
    }
  }

  function closeLocalPlaylist() {
    selectedLocalPlaylist = null;
    localPlaylistError = null;
  }

  async function addLocalTracksToPlaylist(
    playlistId: number,
    libraryTrackIds: number[],
  ) {
    if (libraryTrackIds.length === 0 || localPlaylistBusy) return;
    localPlaylistBusy = true;
    localPlaylistError = null;
    backendError = null;
    try {
      const detail = await addTracksToLocalPlaylist(
        playlistId,
        libraryTrackIds,
      );
      if (selectedLocalPlaylist?.id === playlistId) {
        selectedLocalPlaylist = detail;
      }
      await refreshLocalPlaylistSummaries();
    } catch (error) {
      const message = operationError(
        error,
        'Could not add tracks to the playlist.',
      );
      localPlaylistError = message;
      backendError = message;
    } finally {
      localPlaylistBusy = false;
    }
  }

  async function removeSelectedLocalPlaylistEntry(entryId: number) {
    if (!selectedLocalPlaylist || localPlaylistBusy) return;
    localPlaylistBusy = true;
    localPlaylistError = null;
    try {
      selectedLocalPlaylist = await removeLocalPlaylistEntry(
        selectedLocalPlaylist.id,
        entryId,
      );
      await refreshLocalPlaylistSummaries();
    } catch (error) {
      localPlaylistError = operationError(error, 'Could not remove the track.');
    } finally {
      localPlaylistBusy = false;
    }
  }

  async function moveSelectedLocalPlaylistEntry(
    entryId: number,
    newPosition: number,
  ) {
    if (!selectedLocalPlaylist || localPlaylistBusy) return;
    localPlaylistBusy = true;
    localPlaylistError = null;
    try {
      selectedLocalPlaylist = await moveLocalPlaylistEntry(
        selectedLocalPlaylist.id,
        entryId,
        newPosition,
      );
    } catch (error) {
      localPlaylistError = operationError(
        error,
        'Could not reorder the track.',
      );
    } finally {
      localPlaylistBusy = false;
    }
  }

  async function loadIssues(append = false) {
    const offset = append ? issues.length : 0;
    if (append) {
      issuesLoadingMore = true;
    } else {
      issuesLoading = true;
      issueError = null;
    }
    try {
      const page = await listIssues(offset, issuePageSize);
      issues = append ? [...issues, ...page.items] : page.items;
      issueTotal = page.total;
      issueCounts = page.counts;
      issuesLoaded = true;

      if (!append) {
        const selected =
          issues.find((issue) => issue.id === selectedIssueId) ??
          issues[0] ??
          null;
        selectedIssueId = selected?.id ?? null;
        if (activeView === 'issues' && selected?.kind === 'matchReview') {
          await loadMatchReview(selected);
        } else if (!selected || selected.kind !== 'matchReview') {
          matchReview = null;
        }
      }
    } catch (error) {
      issueError = operationError(error, 'Could not load unresolved issues.');
    } finally {
      issuesLoading = false;
      issuesLoadingMore = false;
    }
  }

  async function loadMoreIssues() {
    if (issuesLoadingMore || issues.length >= issueTotal) {
      return;
    }
    await loadIssues(true);
  }

  async function loadStaging(silent = false) {
    if (!silent) {
      stagingLoading = true;
      stagingError = null;
    }
    try {
      const [page, syncRuns] = await Promise.all([
        listStagingItems(0, stagingPageSize),
        listSyncRuns(null, 0, 10),
      ]);
      if (!sameStagingItems(stagingItems, page.items)) {
        stagingItems = page.items;
      }
      if (stagingTotal !== page.total) stagingTotal = page.total;
      stagingLoaded = true;
      recentSyncRuns = syncRuns.items;
      const nextLocalRun =
        syncRuns.items.find((run) => run.scope === 'local') ?? null;
      const nextSpotifyRun =
        syncRuns.items.find((run) => run.scope === 'spotify') ?? null;
      if (!sameSyncRun(localSyncRun, nextLocalRun)) localSyncRun = nextLocalRun;
      if (!sameSyncRun(spotifySyncRun, nextSpotifyRun)) {
        spotifySyncRun = nextSpotifyRun;
      }
      const availableTrackIds = new Set(
        stagingItems.map((item) => item.libraryTrackId),
      );
      const nextSelectedTrackIds = selectedStagingTrackIds.filter((trackId) =>
        availableTrackIds.has(trackId),
      );
      if (nextSelectedTrackIds.length !== selectedStagingTrackIds.length) {
        selectedStagingTrackIds = nextSelectedTrackIds;
      }
      const selected =
        stagingItems.find(
          (item) => item.libraryTrackId === selectedStagingTrackId,
        ) ??
        stagingItems[0] ??
        null;
      const nextSelectedTrackId = selected?.libraryTrackId ?? null;
      if (selectedStagingTrackId !== nextSelectedTrackId) {
        selectedStagingTrackId = nextSelectedTrackId;
      }
    } catch (error) {
      stagingError = operationError(error, 'Could not load the staging queue.');
    } finally {
      if (!silent) stagingLoading = false;
    }
  }

  function selectStagingItem(item: StagingItem) {
    selectedStagingTrackId = item.libraryTrackId;
  }

  function setStagingSelection(trackIds: number[]) {
    const available = new Set(stagingItems.map((item) => item.libraryTrackId));
    selectedStagingTrackIds = [...new Set(trackIds)].filter((trackId) =>
      available.has(trackId),
    );
  }

  function selectedStagingItems(): StagingItem[] {
    const selectedIds = new Set(selectedStagingTrackIds);
    return stagingItems.filter((item) => selectedIds.has(item.libraryTrackId));
  }

  async function runStagingAction(
    action: () => Promise<void>,
    fallback: string,
  ) {
    if (stagingBusy) return;
    stagingBusy = true;
    stagingError = null;
    try {
      await action();
    } catch (error) {
      stagingError = operationError(error, fallback);
    } finally {
      stagingBusy = false;
      await loadStaging(true);
    }
  }

  function startSelectedStagingTracks() {
    const startable = selectedStagingItems().filter((item) =>
      ['needsLocalCopy', 'needsResolution', 'failed', 'cancelled'].includes(
        item.stage,
      ),
    );
    if (startable.length === 0) return;
    void runStagingAction(async () => {
      for (const item of startable) {
        await startStagingTrack(item.libraryTrackId);
      }
    }, 'Could not start the selected tracks.');
  }

  function cancelSelectedStagingTracks() {
    const jobIds = selectedStagingItems()
      .filter((item) =>
        ['queued', 'searching', 'downloading'].includes(item.stage),
      )
      .flatMap((item) => (item.jobId === null ? [] : [item.jobId]));
    if (jobIds.length === 0) return;
    void runStagingAction(async () => {
      for (const jobId of jobIds) await cancelStagingTrack(jobId);
    }, 'Could not cancel the selected downloads.');
  }

  function processSelectedStagingTracks() {
    const jobIds = selectedStagingItems()
      .filter((item) => item.stage === 'downloaded')
      .flatMap((item) => (item.jobId === null ? [] : [item.jobId]));
    if (jobIds.length === 0) return;
    void runStagingAction(async () => {
      const result = await continueStagingTracks(jobIds);
      if (result.failed > 0) {
        const firstFailure = result.results.find((item) => !item.imported);
        const detail = firstFailure?.errorMessage
          ? ` ${firstFailure.errorMessage}`
          : '';
        throw new Error(
          `${result.failed} selected ${result.failed === 1 ? 'track could' : 'tracks could'} not be verified or imported.${detail}`,
        );
      }
    }, 'Could not process the selected downloads.');
  }

  function excludeStagingTracksFromTracking(libraryTrackIds: number[]) {
    const availableIds = new Set(
      stagingItems.map((item) => item.libraryTrackId),
    );
    const trackIds = [...new Set(libraryTrackIds)].filter((trackId) =>
      availableIds.has(trackId),
    );
    if (trackIds.length === 0) return;

    void runStagingAction(async () => {
      for (const trackId of trackIds) {
        await excludeStagingTrackFromTracking(trackId);
      }
      await hydrateSourceState(false);
    }, 'Could not exclude the selected tracks from tracking.');
  }

  async function resolveOneStagingTrack(
    jobId: number,
    provider: string,
    providerToken: string,
  ) {
    stagingError = null;
    try {
      await resolveStagingTrack(jobId, provider, providerToken);
    } catch (error) {
      stagingError = operationError(
        error,
        'Could not download the selected candidate.',
      );
    } finally {
      await loadStaging(true);
    }
  }

  async function rejectOneStagingCandidate(
    jobId: number,
    provider: string,
    providerToken: string,
  ) {
    stagingError = null;
    try {
      await rejectStagingCandidate(jobId, provider, providerToken);
      await loadStaging(true);
    } catch (error) {
      stagingError = operationError(error, 'Could not reject this candidate.');
    }
  }

  async function searchStagingAgain(jobId: number) {
    stagingError = null;
    try {
      await searchAgainStagingTrack(jobId);
    } catch (error) {
      stagingError = operationError(
        error,
        'Could not search for new acquisition candidates.',
      );
    } finally {
      await loadStaging(true);
    }
  }

  function latestStagingSyncRun(
    localRun: SyncRun | null,
    spotifyRun: SyncRun | null,
  ): SyncRun | null {
    const runs = [localRun, spotifyRun].filter(
      (run): run is SyncRun => run !== null,
    );
    if (runs.length === 0) return null;
    return runs.reduce((latest, run) =>
      run.startedAt > latest.startedAt ? run : latest,
    );
  }

  function sameSyncRun(left: SyncRun | null, right: SyncRun | null): boolean {
    if (left === right) return true;
    if (!left || !right) return false;
    return (
      left.id === right.id &&
      left.scope === right.scope &&
      left.trigger === right.trigger &&
      left.status === right.status &&
      left.phase === right.phase &&
      left.startedAt === right.startedAt &&
      left.finishedAt === right.finishedAt &&
      left.sourceAdded === right.sourceAdded &&
      left.sourceRemoved === right.sourceRemoved &&
      left.matched === right.matched &&
      left.missing === right.missing &&
      left.needsReview === right.needsReview &&
      left.acquisitionFailed === right.acquisitionFailed &&
      left.errorMessage === right.errorMessage
    );
  }

  function sameStagingItems(
    left: StagingItem[],
    right: StagingItem[],
  ): boolean {
    if (left === right) return true;
    if (left.length !== right.length) return false;
    for (let index = 0; index < left.length; index += 1) {
      const current = left[index];
      const next = right[index];
      if (
        current.libraryTrackId !== next.libraryTrackId ||
        current.jobId !== next.jobId ||
        current.title !== next.title ||
        current.album !== next.album ||
        current.durationMs !== next.durationMs ||
        current.imageUrl !== next.imageUrl ||
        current.provider !== next.provider ||
        current.providerJobId !== next.providerJobId ||
        current.stage !== next.stage ||
        current.jobStatus !== next.jobStatus ||
        current.attempt !== next.attempt ||
        current.stagingPath !== next.stagingPath ||
        current.errorCode !== next.errorCode ||
        current.errorMessage !== next.errorMessage ||
        current.bytesTransferred !== next.bytesTransferred ||
        current.totalBytes !== next.totalBytes ||
        current.createdAt !== next.createdAt ||
        current.startedAt !== next.startedAt ||
        current.finishedAt !== next.finishedAt ||
        current.updatedAt !== next.updatedAt ||
        current.artists.length !== next.artists.length ||
        current.artists.some(
          (artist, artistIndex) => artist !== next.artists[artistIndex],
        ) ||
        !sameAcquisitionCandidate(current.candidate, next.candidate) ||
        current.candidates.length !== next.candidates.length ||
        current.candidates.some(
          (candidate, candidateIndex) =>
            !sameAcquisitionCandidate(
              candidate,
              next.candidates[candidateIndex],
            ),
        )
      ) {
        return false;
      }
    }
    return true;
  }

  function sameAcquisitionCandidate(
    left: StagingItem['candidate'],
    right: StagingItem['candidate'],
  ): boolean {
    if (left === right) return true;
    if (!left || !right) return false;
    return (
      left.providerToken === right.providerToken &&
      left.source === right.source &&
      left.fileName === right.fileName &&
      left.title === right.title &&
      left.album === right.album &&
      left.durationMs === right.durationMs &&
      left.format === right.format &&
      left.sizeBytes === right.sizeBytes &&
      left.isrc === right.isrc &&
      left.recordingId === right.recordingId &&
      left.releaseId === right.releaseId &&
      left.artists.length === right.artists.length &&
      left.artists.every((artist, index) => artist === right.artists[index])
    );
  }

  async function selectIssue(issue: IssueRow) {
    selectedIssueId = issue.id;
    if (issue.kind === 'matchReview') {
      await loadMatchReview(issue);
    } else {
      matchReview = null;
    }
  }

  async function loadMatchReview(issue: IssueRow) {
    if (issue.sourceTrackId === null) {
      matchReview = null;
      return;
    }
    matchReviewLoading = true;
    issueError = null;
    try {
      matchReview = await getMatchReview(issue.sourceTrackId);
    } catch (error) {
      matchReview = null;
      issueError = operationError(error, 'Could not load match candidates.');
    } finally {
      matchReviewLoading = false;
    }
  }

  async function confirmIssueMatch(
    sourceTrackId: number,
    libraryTrackId: number,
  ) {
    matchDecisionBusy = true;
    issueError = null;
    try {
      await confirmMatch(sourceTrackId, libraryTrackId);
      await Promise.all([loadIssues(), loadLibraryTracks(), loadStaging(true)]);
    } catch (error) {
      issueError = operationError(error, 'Could not confirm the match.');
    } finally {
      matchDecisionBusy = false;
    }
  }

  async function rejectIssueMatch(
    sourceTrackId: number,
    libraryTrackId: number,
  ) {
    matchDecisionBusy = true;
    issueError = null;
    try {
      await rejectMatch(sourceTrackId, libraryTrackId);
      await Promise.all([loadIssues(), loadStaging(true)]);
    } catch (error) {
      issueError = operationError(error, 'Could not reject the match.');
    } finally {
      matchDecisionBusy = false;
    }
  }

  async function clearIssueRejection(
    sourceTrackId: number,
    libraryTrackId: number,
  ) {
    matchDecisionBusy = true;
    issueError = null;
    try {
      await clearMatchDecision(sourceTrackId, libraryTrackId);
      await Promise.all([loadIssues(), loadStaging(true)]);
    } catch (error) {
      issueError = operationError(error, 'Could not clear the match decision.');
    } finally {
      matchDecisionBusy = false;
    }
  }

  async function trashInvalidIssue(localFileId: number) {
    issueTrashBusyId = localFileId;
    issueError = null;
    try {
      await trashInvalidLocalFile(localFileId);
      const [, , overview] = await Promise.all([
        loadIssues(),
        loadLibraryTracks(),
        getLocalLibraryOverview(),
      ]);
      libraryOverview = overview;
    } catch (error) {
      issueError = operationError(
        error,
        'Could not move the invalid file to Trash.',
      );
    } finally {
      issueTrashBusyId = null;
    }
  }

  async function selectSavedAlbum(album: SourceCollectionSummary) {
    selectedSavedAlbum = album;
    await loadCollection(album);
  }

  async function selectPlaylist(playlist: SourceCollectionSummary) {
    selectedPlaylist = playlist;
    await loadCollection(playlist);
  }

  async function updateCollectionTracking(
    collection: SourceCollectionSummary,
    included: boolean,
  ) {
    trackingBusyId = -collection.id;
    collectionError = null;
    try {
      await setSourceCollectionTracking(collection.id, included);
      await Promise.all([
        hydrateSourceState(),
        loadLocalPlaylists(),
        loadStaging(true),
      ]);
    } catch (error) {
      collectionError = operationError(
        error,
        'Could not update Spotify tracking.',
      );
    } finally {
      trackingBusyId = null;
    }
  }

  async function updateTrackTracking(
    entry: SourceCollectionEntryView,
    included: boolean | null,
  ) {
    if (!currentCollection || !entry.track) return;
    const collection = currentCollection;
    let updated = false;
    trackingBusyId = entry.track.id;
    collectionError = null;
    try {
      await setSourceTrackTracking(collection.id, entry.track.id, included);
      await loadCollection(collection);
      updated = true;
    } catch (error) {
      collectionError = operationError(
        error,
        'Could not update track tracking.',
      );
    } finally {
      trackingBusyId = null;
    }

    if (updated) {
      void Promise.all([
        hydrateSourceState(false),
        loadLocalPlaylists(),
        loadStaging(true),
      ]);
    }
  }

  async function updateTracksTracking(
    entries: SourceCollectionEntryView[],
    included: boolean | null,
  ) {
    if (!currentCollection) return;
    const collection = currentCollection;
    const sourceTrackIds = [
      ...new Set(
        entries.flatMap((entry) => (entry.track ? [entry.track.id] : [])),
      ),
    ];
    if (sourceTrackIds.length === 0) return;

    let updated = false;
    trackingBulkBusy = true;
    collectionError = null;
    try {
      await setSourceTracksTracking(collection.id, sourceTrackIds, included);
      await loadCollection(collection);
      updated = true;
    } catch (error) {
      collectionError = operationError(
        error,
        'Could not update selected track tracking.',
      );
    } finally {
      trackingBulkBusy = false;
    }

    if (updated) {
      void Promise.all([
        hydrateSourceState(false),
        loadLocalPlaylists(),
        loadStaging(true),
      ]);
    }
  }

  async function loadCollection(
    collection: SourceCollectionSummary,
    append = false,
  ) {
    currentCollection = collection;
    collectionError = null;

    if (!collection.isAccessible) {
      collectionEntries = [];
      collectionTotal = collection.entryCount;
      loadedCollectionId = collection.id;
      collectionLoading = false;
      collectionLoadingMore = false;
      return;
    }

    const offset = append ? collectionEntries.length : 0;
    if (append) {
      collectionLoadingMore = true;
    } else {
      collectionLoading = true;
      loadedCollectionId = null;
      collectionEntries = [];
      collectionTotal = collection.entryCount;
    }

    try {
      const page = await getSourceCollectionPage(
        collection.id,
        offset,
        collectionPageSize,
      );
      if (currentCollection?.id !== collection.id) {
        return;
      }
      currentCollection = page.collection;
      collectionTotal = page.total;
      collectionEntries = append
        ? [...collectionEntries, ...page.entries]
        : page.entries;
      loadedCollectionId = collection.id;
    } catch (error) {
      collectionError = spotifyErrorMessage(error);
    } finally {
      collectionLoading = false;
      collectionLoadingMore = false;
    }
  }

  async function loadMoreCollection() {
    if (!currentCollection || collectionLoadingMore) {
      return;
    }
    await loadCollection(currentCollection, true);
  }

  async function loadMoreSavedAlbums() {
    if (savedAlbumsLoadingMore || savedAlbums.length >= savedAlbumTotal) {
      return;
    }
    savedAlbumsLoadingMore = true;
    sourceHydrationError = null;
    try {
      const page = await listSpotifySavedAlbums(
        savedAlbums.length,
        collectionListPageSize,
      );
      savedAlbums = [...savedAlbums, ...page.items];
      savedAlbumTotal = page.total;
    } catch (error) {
      sourceHydrationError = spotifyErrorMessage(error);
    } finally {
      savedAlbumsLoadingMore = false;
    }
  }

  async function loadMorePlaylists() {
    if (playlistsLoadingMore || playlists.length >= playlistTotal) {
      return;
    }
    playlistsLoadingMore = true;
    sourceHydrationError = null;
    try {
      const page = await listSpotifyPlaylists(
        playlists.length,
        collectionListPageSize,
      );
      playlists = [...playlists, ...page.items];
      playlistTotal = page.total;
    } catch (error) {
      sourceHydrationError = spotifyErrorMessage(error);
    } finally {
      playlistsLoadingMore = false;
    }
  }

  function formatSyncTime(value: number | null | undefined): string {
    if (!value) return 'Never';
    return new Intl.DateTimeFormat(undefined, {
      month: 'short',
      day: 'numeric',
      hour: 'numeric',
      minute: '2-digit',
    }).format(new Date(value));
  }

  function operationError(error: unknown, fallback: string): string {
    if (error instanceof Error && error.message) {
      return error.message;
    }
    if (
      typeof error === 'object' &&
      error !== null &&
      'message' in error &&
      typeof error.message === 'string'
    ) {
      return error.message;
    }
    if (typeof error === 'string' && error.trim()) {
      return error;
    }
    return fallback;
  }

  function previousSidebarFeedback() {
    if (sidebarFeedback.length <= 1) return;
    sidebarFeedbackIndex =
      (sidebarFeedbackIndex - 1 + sidebarFeedback.length) %
      sidebarFeedback.length;
  }

  function nextSidebarFeedback() {
    if (sidebarFeedback.length <= 1) return;
    sidebarFeedbackIndex = (sidebarFeedbackIndex + 1) % sidebarFeedback.length;
  }

  function runSidebarFeedbackAction(feedback: SidebarFeedback) {
    if (feedback.action === 'cancelSync') {
      void cancelSynchronization();
    } else if (feedback.action === 'cancelSource') {
      void cancelSourceRefresh();
    }
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && sidebarFeedbackDialog) {
      sidebarFeedbackDialog = null;
    }
  }

  function startWindowDrag(event: PointerEvent) {
    if (event.button !== 0) return;
    void getCurrentWindow().startDragging();
  }
</script>

<svelte:head>
  <title>Refrain</title>
</svelte:head>

<svelte:window onkeydown={handleGlobalKeydown} />

<main class="app-shell" class:settings-active={activeView === 'settings'}>
  {#if isMacOS}
    <div
      class="window-drag-region"
      data-tauri-drag-region
      aria-hidden="true"
      onpointerdown={startWindowDrag}
    ></div>
  {/if}
  <aside class="app-sidebar">
    <div class="brand">
      <div class="brand-mark" aria-hidden="true">
        <span class="brand-wave"><i></i><i></i><i></i><i></i><i></i></span>
      </div>
      <div>
        <p class="brand-name">Refrain</p>
        <p class="brand-subtitle">Keep your music in sync</p>
      </div>
    </div>

    <p class="sidebar-label">Library</p>
    <nav class="sidebar-nav" aria-label="Primary navigation">
      <button
        type="button"
        class="nav-row"
        aria-expanded={localExpanded}
        onclick={() => (localExpanded = !localExpanded)}
      >
        <Icon name="local" size={17} />
        <span>Local</span>
        <span class="nav-group-meta">
          <span class="nav-count">{libraryTrackTotal.toLocaleString()}</span>
          <span class="disclosure-chevron" class:expanded={localExpanded}>
            <Icon name="chevron-right" size={13} />
          </span>
        </span>
      </button>
      {#if localExpanded}
        <button
          type="button"
          class:active={activeView === 'library' && localSection === 'songs'}
          class="nav-subrow"
          onclick={() => openLocalSection('songs')}
        >
          <Icon name="local" size={16} />
          <span>Songs</span>
          <span class="nav-count">{libraryTrackTotal.toLocaleString()}</span>
        </button>
        <button
          type="button"
          class:active={activeView === 'library' &&
            localSection === 'playlists'}
          class="nav-subrow"
          onclick={() => openLocalSection('playlists')}
        >
          <Icon name="playlist" size={16} />
          <span>Playlists</span>
          <span class="nav-count">{localPlaylists.length.toLocaleString()}</span
          >
        </button>
      {/if}

      <button
        type="button"
        class="nav-row"
        aria-expanded={spotifyExpanded}
        onclick={() => (spotifyExpanded = !spotifyExpanded)}
      >
        <Icon name="spotify" size={17} />
        <span>Spotify</span>
        <span class="nav-group-meta">
          <span class="disclosure-chevron" class:expanded={spotifyExpanded}>
            <Icon name="chevron-right" size={13} />
          </span>
        </span>
      </button>
      {#if spotifyExpanded}
        <button
          type="button"
          class:active={activeView === 'spotify' && spotifySection === 'liked'}
          class="nav-subrow"
          onclick={() => {
            activeView = 'spotify';
            void loadSpotifySection('liked');
          }}
        >
          <Icon name="heart" size={16} />
          <span>Liked Songs</span>
          <span class="nav-count"
            >{(
              sourceOverview?.likedSongs?.entryCount ?? 0
            ).toLocaleString()}</span
          >
        </button>
        <button
          type="button"
          class:active={activeView === 'spotify' && spotifySection === 'albums'}
          class="nav-subrow"
          onclick={() => {
            activeView = 'spotify';
            void loadSpotifySection('albums');
          }}
        >
          <Icon name="album" size={16} />
          <span>Albums</span>
          <span class="nav-count">{savedAlbumTotal.toLocaleString()}</span>
        </button>
        <button
          type="button"
          class:active={activeView === 'spotify' &&
            spotifySection === 'playlists'}
          class="nav-subrow"
          onclick={() => {
            activeView = 'spotify';
            void loadSpotifySection('playlists');
          }}
        >
          <Icon name="playlist" size={16} />
          <span>Playlists</span>
          <span class="nav-count"
            >{(
              sourceOverview?.playlistCount ?? playlistTotal
            ).toLocaleString()}</span
          >
        </button>
      {/if}

      <button
        type="button"
        class:active={activeView === 'staging'}
        class="nav-row"
        onclick={() => openView('staging')}
      >
        <Icon name="download" size={17} />
        <span>Staging</span>
        <span class="nav-count">{stagingTotal.toLocaleString()}</span>
      </button>

      <button
        type="button"
        class:active={activeView === 'issues'}
        class="nav-row"
        onclick={() => openView('issues')}
      >
        <Icon name="issues" size={17} />
        <span>Issues</span>
        <span class="nav-count">{issueTotal.toLocaleString()}</span>
      </button>
    </nav>

    <div class="sidebar-spacer"></div>
    <div class="sidebar-footer">
      {#if currentSidebarFeedback}
        <div class="sidebar-feedback-carousel" aria-live="polite">
          <div
            class:error={currentSidebarFeedback.kind === 'error'}
            class:progress={currentSidebarFeedback.kind === 'progress'}
            class:success={currentSidebarFeedback.kind === 'success'}
            class="sidebar-feedback-card"
          >
            <button
              type="button"
              class="sidebar-feedback-open"
              onclick={() => (sidebarFeedbackDialog = currentSidebarFeedback)}
              title="Open details"
            >
              <span
                class:is-spinning={currentSidebarFeedback.kind === 'progress'}
                class="sidebar-feedback-icon"
              >
                <Icon
                  name={currentSidebarFeedback.kind === 'error'
                    ? 'warning'
                    : currentSidebarFeedback.kind === 'success'
                      ? 'check'
                      : 'refresh'}
                  size={13}
                />
              </span>
              <span class="sidebar-feedback-copy">
                <strong>{currentSidebarFeedback.title}</strong>
                <span>{currentSidebarFeedback.message}</span>
              </span>
            </button>
            {#if sidebarFeedback.length > 1 || currentSidebarFeedback.action}
              <div class="sidebar-feedback-controls">
                <span
                  >{sidebarFeedbackIndex + 1} of {sidebarFeedback.length}</span
                >
                <div>
                  {#if currentSidebarFeedback.action}
                    <button
                      type="button"
                      class="sidebar-feedback-action"
                      onclick={() =>
                        runSidebarFeedbackAction(currentSidebarFeedback)}
                      >Cancel</button
                    >
                  {/if}
                  {#if sidebarFeedback.length > 1}
                    <button
                      type="button"
                      class="sidebar-feedback-nav"
                      aria-label="Previous notification"
                      onclick={previousSidebarFeedback}
                    >
                      <Icon name="back" size={11} />
                    </button>
                    <button
                      type="button"
                      class="sidebar-feedback-nav"
                      aria-label="Next notification"
                      onclick={nextSidebarFeedback}
                    >
                      <Icon name="chevron-right" size={11} />
                    </button>
                  {/if}
                </div>
              </div>
            {/if}
          </div>
        </div>
      {/if}
      <div class="sidebar-status" aria-label="Spotify synchronization status">
        <span>
          Last sync {formatSyncTime(
            spotifySyncRun?.finishedAt ?? spotifySyncRun?.startedAt,
          )}
        </span>
        <span>
          Last refresh {formatSyncTime(
            sourceOverview?.account?.lastSourceSyncAt,
          )}
        </span>
      </div>
      <div
        class="sidebar-sync-actions"
        aria-label="Library synchronization controls"
      >
        <button
          type="button"
          class="sidebar-action-button"
          onclick={runLibrarySynchronization}
          disabled={!authStatus?.connected ||
            syncBusy ||
            sourceBusy ||
            stagingBusy}
        >
          <Icon name="sync" size={14} />
          {syncBusy ? 'Syncing…' : 'Sync Library'}
        </button>
        <button
          type="button"
          class="sidebar-action-button primary"
          onclick={refreshSource}
          disabled={!authStatus?.connected || sourceBusy || syncBusy}
        >
          <Icon name="refresh" size={14} />
          {sourceBusy ? 'Refreshing…' : 'Refresh Spotify'}
        </button>
      </div>
      <button
        type="button"
        class:active={activeView === 'settings'}
        class="nav-row"
        onclick={() => openView('settings')}
      >
        <Icon name="settings" size={14} />
        <span>Settings</span>
      </button>
    </div>
  </aside>

  <section class="app-main">
    <div class="app-content">
      {#if isSpotifySourceView(activeView) && sourceHydrating && !sourceOverview}
        <div class="screen" aria-label="Loading Spotify library">
          <div class="skeleton" style="height:76px; margin-bottom:12px;"></div>
          <div class="skeleton" style="height:86px; margin-bottom:12px;"></div>
          <div class="skeleton" style="min-height:0; flex:1;"></div>
        </div>
      {:else if isSpotifySourceView(activeView) && sourceHydrationError}
        <div class="error-state">
          <div>
            <strong>Could not load persisted Spotify state.</strong>
            <p style="margin:4px 0 0;">{sourceHydrationError}</p>
          </div>
        </div>
      {:else if activeView === 'library' && localSection === 'songs'}
        <LibraryView
          tracks={libraryTracks}
          total={libraryTrackTotal}
          loading={libraryTracksLoading}
          loadingMore={libraryTracksLoadingMore}
          error={libraryTracksError}
          onLoadMore={loadMoreLibraryTracks}
          onScan={scanLibraryRoot}
          scanBusy={libraryBusy}
          scanDisabled={!libraryRoot.trim()}
          syncRun={localSyncRun}
          indexedFiles={libraryOverview?.total ?? 0}
          localOnlyCount={libraryOverview?.localOnly ?? 0}
          {localPlaylists}
          onAddToPlaylist={addLocalTracksToPlaylist}
        />
      {:else if activeView === 'library' && localSection === 'playlists'}
        <LocalPlaylistsView
          playlists={localPlaylists}
          selectedPlaylist={selectedLocalPlaylist}
          loading={localPlaylistLoading}
          busy={localPlaylistBusy}
          error={localPlaylistError}
          onSelect={selectLocalPlaylist}
          onCreate={createNewLocalPlaylist}
          onBack={closeLocalPlaylist}
          onRename={renameSelectedLocalPlaylist}
          onDelete={deleteSelectedLocalPlaylist}
          onMoveEntry={moveSelectedLocalPlaylistEntry}
          onRemoveEntry={removeSelectedLocalPlaylistEntry}
        />
      {:else if activeView === 'spotify'}
        <SpotifyWorkspace
          bind:detailsSidebarOpen={spotifyDetailsSidebarOpen}
          section={spotifySection}
          overview={sourceOverview}
          {savedAlbums}
          {savedAlbumTotal}
          {playlists}
          {playlistTotal}
          {currentCollection}
          entries={collectionEntries}
          {collectionTotal}
          {collectionLoading}
          {collectionLoadingMore}
          {collectionError}
          {savedAlbumsLoadingMore}
          {playlistsLoadingMore}
          pinnedCollectionKeys={pinnedSpotifyCollections.map(
            pinnedCollectionKey,
          )}
          {trackingBusyId}
          {trackingBulkBusy}
          onSelectSavedAlbum={selectSavedAlbum}
          onSelectPlaylist={selectPlaylist}
          onBackToCollections={closeSpotifyCollection}
          onLoadMoreSavedAlbums={loadMoreSavedAlbums}
          onLoadMorePlaylists={loadMorePlaylists}
          onLoadMoreCollection={loadMoreCollection}
          onSetCollectionTracking={updateCollectionTracking}
          onSetTrackTracking={updateTrackTracking}
          onSetTracksTracking={updateTracksTracking}
          onTogglePinnedCollection={togglePinnedCollection}
        />
      {:else if activeView === 'staging'}
        <StagingView
          items={stagingItems}
          total={stagingTotal}
          selectedLibraryTrackId={selectedStagingTrackId}
          loading={stagingLoading}
          busy={stagingBusy}
          error={stagingError}
          selectedTrackIds={selectedStagingTrackIds}
          syncRun={stagingSyncRun}
          {syncBusy}
          {syncScope}
          onSelect={selectStagingItem}
          onSelectionChange={setStagingSelection}
          onStartSelected={startSelectedStagingTracks}
          onCancelSelected={cancelSelectedStagingTracks}
          onProcessSelected={processSelectedStagingTracks}
          onExclude={excludeStagingTracksFromTracking}
          onResolve={resolveOneStagingTrack}
          onReject={rejectOneStagingCandidate}
          onSearchAgain={searchStagingAgain}
          onCancelSync={cancelSynchronization}
        />
      {:else if activeView === 'issues'}
        <IssuesView
          {issues}
          total={issueTotal}
          counts={issueCounts}
          {selectedIssueId}
          review={matchReview}
          loading={issuesLoading}
          loadingMore={issuesLoadingMore}
          reviewLoading={matchReviewLoading}
          decisionBusy={matchDecisionBusy}
          trashBusyId={issueTrashBusyId}
          error={issueError}
          onSelect={selectIssue}
          onLoadMore={loadMoreIssues}
          onConfirm={confirmIssueMatch}
          onReject={rejectIssueMatch}
          onClearRejection={clearIssueRejection}
          onTrashInvalid={trashInvalidIssue}
        />
      {:else}
        <SettingsView
          {appInfo}
          {authStatus}
          bind:clientId
          {authBusy}
          {authError}
          {sourceBusy}
          {sourceError}
          {sourceSummary}
          {sourceOverview}
          {savedAlbumTotal}
          bind:libraryRoot
          {libraryBusy}
          {libraryError}
          {libraryProgress}
          {librarySummary}
          {libraryOverview}
          bind:acquisitionEnabled
          bind:acquisitionProviders
          bind:soulseekUsername
          bind:soulseekPassword
          {soulseekConfigured}
          {monochromeHealth}
          {monochromeHealthCached}
          {antraConfigured}
          {antraLoginPending}
          {antraUserCode}
          {antraVerificationUrl}
          {antraHealth}
          {antraHealthCached}
          {sockseekHealth}
          {sockseekHealthCached}
          {acquisitionBusy}
          {acquisitionError}
          {syncBusy}
          {syncScope}
          {syncError}
          {localSyncRun}
          {spotifySyncRun}
          {recentSyncRuns}
          {themePreference}
          onConnectSpotify={connect}
          onDisconnectSpotify={disconnect}
          onRefreshSpotify={refreshSource}
          onSyncLibrary={runLibrarySynchronization}
          onPickLibraryRoot={pickLibraryRoot}
          onSaveLibraryRoot={saveLibraryRoot}
          onScanLibrary={scanLibraryRoot}
          onSaveAcquisition={saveAcquisitionSettings}
          onCheckMonochrome={checkMonochromeHealth}
          onCheckAntra={checkAntraHealth}
          onStartAntra={startAntraLogin}
          onOpenAntraLogin={openAntraLoginPage}
          onCancelAntra={cancelAntraLogin}
          onClearAntra={clearAntraAccount}
          onSaveSoulseek={saveSoulseekAccount}
          onCheckSockseek={checkSockseekHealth}
          onClearSoulseek={clearSoulseekAccount}
          onChangeTheme={changeTheme}
        />
      {/if}
    </div>
  </section>

  {#if sidebarFeedbackDialog}
    <div class="sidebar-feedback-modal-layer">
      <button
        type="button"
        class="sidebar-feedback-modal-backdrop"
        aria-label="Close notification details"
        onclick={() => (sidebarFeedbackDialog = null)}
      ></button>
      <div
        class:error={sidebarFeedbackDialog.kind === 'error'}
        class="sidebar-feedback-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="sidebar-feedback-modal-title"
      >
        <div class="sidebar-feedback-modal-header">
          <div>
            <span class="sidebar-feedback-modal-kicker">
              {sidebarFeedbackDialog.kind === 'error'
                ? 'Error'
                : sidebarFeedbackDialog.kind === 'success'
                  ? 'Notification'
                  : 'In progress'}
            </span>
            <h2 id="sidebar-feedback-modal-title">
              {sidebarFeedbackDialog.title}
            </h2>
          </div>
          <button
            type="button"
            class="icon-button"
            aria-label="Close notification details"
            onclick={() => (sidebarFeedbackDialog = null)}
          >
            <Icon name="close" size={15} />
          </button>
        </div>
        <p class="sidebar-feedback-modal-message">
          {sidebarFeedbackDialog.message}
        </p>
        {#if sidebarFeedbackDialog.action}
          <div class="sidebar-feedback-modal-actions">
            <button
              type="button"
              class="btn"
              onclick={() =>
                sidebarFeedbackDialog &&
                runSidebarFeedbackAction(sidebarFeedbackDialog)}
              >Cancel operation</button
            >
          </div>
        {/if}
      </div>
    </div>
  {/if}
</main>
