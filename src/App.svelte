<script lang="ts">
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount } from 'svelte';
  import Icon from './components/Icon.svelte';
  import IssuesView from './components/IssuesView.svelte';
  import LibraryView from './components/LibraryView.svelte';
  import SpotifyWorkspace from './components/SpotifyWorkspace.svelte';
  import { getAppInfo, type AppInfo } from './lib/app-info';
  import { chooseLibraryRoot } from './lib/dialog';
  import {
    getMatchReview,
    listIssues,
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
  import { getSettings, updateSettings } from './lib/settings';
  import {
    clearSoulseekCredentials,
    getSockseekProviderHealth,
    getSoulseekCredentialStatus,
    setSoulseekCredentials,
    type ProviderHealth,
  } from './lib/sockseek';
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
    type SyncRun,
  } from './lib/sync';

  type AppView = 'library' | 'spotify' | 'issues' | 'settings';
  type SpotifySection = 'liked' | 'albums' | 'playlists';

  const fallbackRedirectUri = 'http://127.0.0.1:43817/callback';
  const sourceProgressEvent = 'spotify-source-refresh-progress';
  const libraryProgressEvent = 'local-library-scan-progress';
  const collectionPageSize = 200;
  const collectionListPageSize = 100;
  const libraryTrackPageSize = 100;
  const issuePageSize = 100;

  let activeView: AppView = 'library';
  let spotifySection: SpotifySection = 'liked';
  let appInfo: AppInfo | null = null;
  let authStatus: SpotifyAuthStatus | null = null;
  let clientId = '';
  let acquisitionEnabled = false;
  let soulseekUsername = '';
  let soulseekPassword = '';
  let soulseekConfigured = false;
  let sockseekHealth: ProviderHealth | null = null;
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
  let syncRun: SyncRun | null = null;
  let localSyncRun: SyncRun | null = null;
  let spotifySyncRun: SyncRun | null = null;
  let syncError: string | null = null;
  let trackingBusyId: number | null = null;
  let trackingBulkBusy = false;
  let sourceOverview: SpotifySourceOverview | null = null;
  let sourceHydrating = true;
  let sourceHydrationError: string | null = null;
  let savedAlbums: SourceCollectionSummary[] = [];
  let savedAlbumTotal = 0;
  let savedAlbumsLoadingMore = false;
  let selectedSavedAlbum: SourceCollectionSummary | null = null;
  let playlists: SourceCollectionSummary[] = [];
  let playlistTotal = 0;
  let playlistsLoadingMore = false;
  let selectedPlaylist: SourceCollectionSummary | null = null;
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
  $: actionableIssues = actionableIssueRows(issues);
  $: actionableIssueTotal =
    issueCounts.localOnlyTrack +
    issueCounts.missingLocalFile +
    issueCounts.matchReview;
  $: actionableIssueCounts = {
    ...issueCounts,
    inaccessibleCollection: 0,
    invalidLocalFile: 0,
    acquisitionFailed: 0,
  };
  let issuesLoading = false;
  let issuesLoadingMore = false;
  let issueError: string | null = null;
  let selectedIssueId: string | null = null;
  let matchReview: MatchReview | null = null;
  let matchReviewLoading = false;
  let matchDecisionBusy = false;
  let isMacOS = false;

  onMount(() => {
    isMacOS = navigator.userAgent.includes('Macintosh');
    let disposed = false;
    let sourceUnlisten: UnlistenFn | undefined;
    let libraryUnlisten: UnlistenFn | undefined;

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
    void initialize();

    return () => {
      disposed = true;
      sourceUnlisten?.();
      libraryUnlisten?.();
    };
  });

  async function initialize() {
    try {
      const [info, status, settings, localOverview, soulseekStatus] =
        await Promise.all([
          getAppInfo(),
          getSpotifyAuthStatus(),
          getSettings(),
          getLocalLibraryOverview(),
          getSoulseekCredentialStatus(),
        ]);
      appInfo = info;
      authStatus = status;
      clientId = status.clientId ?? '';
      libraryRoot = settings.libraryRoot ?? '';
      acquisitionEnabled = settings.acquisitionEnabled;
      soulseekConfigured = soulseekStatus.configured;
      soulseekUsername = soulseekStatus.username ?? '';
      libraryOverview = localOverview;
      backendError = null;
      await hydrateSourceState();
      const [localRuns, spotifyRuns] = await Promise.all([
        listSyncRuns('local', 0, 1),
        listSyncRuns('spotify', 0, 1),
      ]);
      localSyncRun = localRuns.items[0] ?? null;
      spotifySyncRun = spotifyRuns.items[0] ?? null;
      await Promise.all([loadLibraryTracks(), loadIssues()]);
    } catch (error) {
      backendError = spotifyErrorMessage(error);
      sourceHydrating = false;
    }
  }

  async function hydrateSourceState() {
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

      if (activeView === 'spotify') {
        await loadSpotifySection(spotifySection);
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
      await loadIssues();
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
    if (syncBusy || sourceBusy || authBusy) return;

    syncBusy = true;
    syncScope = 'local';
    syncRun = null;
    syncError = null;
    try {
      const localRun = await startLocalSync('manual');
      syncRun = localRun;
      localSyncRun = localRun;
      if (localRun.status === 'failed') {
        syncError = localRun.errorMessage ?? 'Local reconciliation failed.';
        return;
      }
      if (localRun.status === 'cancelled') return;

      syncScope = 'spotify';
      const spotifyRun = await startSpotifySync('manual');
      syncRun = spotifyRun;
      spotifySyncRun = spotifyRun;
      if (spotifyRun.status === 'failed') {
        syncError =
          spotifyRun.errorMessage ?? 'Library synchronization failed.';
      }

      libraryOverview = await getLocalLibraryOverview();
      await Promise.all([
        hydrateSourceState(),
        loadLibraryTracks(),
        loadIssues(),
      ]);
    } catch (error) {
      syncError = operationError(error, 'Could not synchronize the library.');
    } finally {
      syncBusy = false;
      syncScope = null;
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

  async function saveAcquisitionSettings() {
    acquisitionBusy = true;
    acquisitionError = null;
    try {
      if (acquisitionEnabled && !soulseekConfigured) {
        throw new Error(
          'Save your Soulseek account credentials before enabling acquisition.',
        );
      }
      const current = await getSettings();
      await updateSettings({ ...current, acquisitionEnabled });
    } catch (error) {
      acquisitionError = operationError(
        error,
        'Could not save acquisition settings.',
      );
    } finally {
      acquisitionBusy = false;
    }
  }

  async function saveSoulseekAccount() {
    acquisitionBusy = true;
    acquisitionError = null;
    sockseekHealth = null;
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
    try {
      const status = await clearSoulseekCredentials();
      soulseekConfigured = status.configured;
      soulseekUsername = status.username ?? '';
      soulseekPassword = '';
      if (acquisitionEnabled) {
        const current = await getSettings();
        acquisitionEnabled = false;
        await updateSettings({ ...current, acquisitionEnabled: false });
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
    try {
      sockseekHealth = await getSockseekProviderHealth();
    } catch (error) {
      acquisitionError = operationError(
        error,
        'Could not start the Sockseek provider.',
      );
    } finally {
      acquisitionBusy = false;
    }
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
      await Promise.all([loadLibraryTracks(), loadIssues()]);
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
    activeView = view;
    collectionError = null;

    if (view === 'library') {
      void loadLibraryTracks();
    } else if (view === 'spotify') {
      void loadSpotifySection(spotifySection);
    } else if (view === 'issues') {
      void loadIssues();
    }
  }

  function isSpotifySourceView(view: AppView): boolean {
    return view === 'spotify';
  }

  async function loadSpotifySection(section: SpotifySection) {
    const sectionChanged = spotifySection !== section;
    spotifySection = section;
    collectionError = null;

    if (sectionChanged) {
      if (section === 'albums') selectedSavedAlbum = null;
      if (section === 'playlists') selectedPlaylist = null;
    }

    if (section === 'liked' && sourceOverview?.likedSongs) {
      await loadCollection(sourceOverview.likedSongs);
    } else if (section === 'albums' && selectedSavedAlbum) {
      await loadCollection(selectedSavedAlbum);
    } else if (section === 'playlists' && selectedPlaylist) {
      await loadCollection(selectedPlaylist);
    } else {
      currentCollection = null;
      collectionEntries = [];
      collectionTotal = 0;
    }
  }

  function closeSpotifyCollection() {
    if (spotifySection === 'albums') selectedSavedAlbum = null;
    if (spotifySection === 'playlists') selectedPlaylist = null;
    currentCollection = null;
    collectionEntries = [];
    collectionTotal = 0;
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

      if (!append) {
        const currentActionableIssues = actionableIssueRows(issues);
        const selected =
          currentActionableIssues.find(
            (issue) => issue.id === selectedIssueId,
          ) ??
          currentActionableIssues[0] ??
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

  function actionableIssueRows(rows: IssueRow[]): IssueRow[] {
    return rows.filter(
      (issue) =>
        issue.kind === 'localOnlyTrack' ||
        issue.kind === 'missingLocalFile' ||
        issue.kind === 'matchReview',
    );
  }

  async function loadMoreIssues() {
    if (issuesLoadingMore || issues.length >= issueTotal) {
      return;
    }
    await loadIssues(true);
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
      await Promise.all([loadIssues(), loadLibraryTracks()]);
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
      await loadIssues();
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
      await loadIssues();
    } catch (error) {
      issueError = operationError(error, 'Could not clear the match decision.');
    } finally {
      matchDecisionBusy = false;
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
      await hydrateSourceState();
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
    trackingBusyId = entry.track.id;
    collectionError = null;
    try {
      await setSourceTrackTracking(
        currentCollection.id,
        entry.track.id,
        included,
      );
      await loadCollection(currentCollection);
      await hydrateSourceState();
    } catch (error) {
      collectionError = operationError(
        error,
        'Could not update track tracking.',
      );
    } finally {
      trackingBusyId = null;
    }
  }

  async function updateTracksTracking(
    entries: SourceCollectionEntryView[],
    included: boolean | null,
  ) {
    if (!currentCollection) return;
    const sourceTrackIds = [
      ...new Set(
        entries.flatMap((entry) => (entry.track ? [entry.track.id] : [])),
      ),
    ];
    if (sourceTrackIds.length === 0) return;

    trackingBulkBusy = true;
    collectionError = null;
    try {
      await setSourceTracksTracking(
        currentCollection.id,
        sourceTrackIds,
        included,
      );
      await loadCollection(currentCollection);
      await hydrateSourceState();
    } catch (error) {
      collectionError = operationError(
        error,
        'Could not update selected track tracking.',
      );
    } finally {
      trackingBulkBusy = false;
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
      collectionLoading = false;
      collectionLoadingMore = false;
      return;
    }

    const offset = append ? collectionEntries.length : 0;
    if (append) {
      collectionLoadingMore = true;
    } else {
      collectionLoading = true;
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

  function startWindowDrag(event: PointerEvent) {
    if (event.button !== 0) return;
    void getCurrentWindow().startDragging();
  }
</script>

<svelte:head>
  <title>Refrain</title>
</svelte:head>

<main class="app-shell">
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
        class:active={activeView === 'library'}
        class="nav-row"
        onclick={() => openView('library')}
      >
        <Icon name="local" size={17} />
        <span>Local</span>
        <span class="nav-count">{libraryTrackTotal.toLocaleString()}</span>
      </button>

      <button
        type="button"
        class:active={activeView === 'spotify'}
        class="nav-row"
        onclick={() => openView('spotify')}
      >
        <Icon name="spotify" size={17} />
        <span>Spotify</span>
        <span style="margin-left:auto"
          ><Icon name="chevron-down" size={13} /></span
        >
      </button>
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

      <button
        type="button"
        class:active={activeView === 'issues'}
        class="nav-row"
        onclick={() => openView('issues')}
      >
        <Icon name="issues" size={17} />
        <span>Issues</span>
        <span class="nav-count">{actionableIssueTotal.toLocaleString()}</span>
      </button>
    </nav>

    <div class="sidebar-spacer"></div>
    <div class="sidebar-footer">
      {#if syncError || sourceError || backendError}
        <div class="sidebar-alerts" aria-live="polite">
          {#if syncError}
            <div class="sidebar-alert" title={syncError}>
              <Icon name="warning" size={13} />
              <span>{syncError}</span>
            </div>
          {/if}
          {#if sourceError}
            <div class="sidebar-alert" title={sourceError}>
              <Icon name="warning" size={13} />
              <span>{sourceError}</span>
            </div>
          {/if}
          {#if backendError}
            <div class="sidebar-alert" title={backendError}>
              <Icon name="warning" size={13} />
              <span>{backendError}</span>
            </div>
          {/if}
        </div>
      {/if}
      {#if sourceBusy && sourceProgress}
        <div class="sidebar-progress" aria-live="polite">
          <span title={sourceProgress.message}>
            {sourceProgress.message}{sourceProgress.total !== null
              ? ` · ${sourceProgress.completed}/${sourceProgress.total}`
              : ''}
          </span>
          <button type="button" onclick={cancelSourceRefresh}>Cancel</button>
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
          disabled={!authStatus?.connected || syncBusy || sourceBusy}
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
        <Icon name="settings" size={17} />
        <span>Settings</span>
      </button>
    </div>
  </aside>

  <section class="app-main">
    <div class="status-stack">
      {#if syncBusy}
        <div class="status-banner">
          <span
            >{syncScope === 'local'
              ? 'Reconciling local files…'
              : 'Syncing tracked music…'}</span
          >
          <button type="button" class="btn" onclick={cancelSynchronization}
            >Cancel</button
          >
        </div>
      {/if}
    </div>

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
      {:else if activeView === 'library'}
        <LibraryView
          tracks={libraryTracks}
          total={libraryTrackTotal}
          loading={libraryTracksLoading}
          loadingMore={libraryTracksLoadingMore}
          error={libraryTracksError}
          onLoadMore={loadMoreLibraryTracks}
          onScan={scanLibraryRoot}
          onShowIssues={() => openView('issues')}
          scanBusy={libraryBusy}
          scanDisabled={!libraryRoot.trim()}
          syncRun={localSyncRun}
          indexedFiles={libraryOverview?.total ?? 0}
          issueCount={actionableIssueCounts.localOnlyTrack}
        />
      {:else if activeView === 'spotify'}
        <SpotifyWorkspace
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
        />
      {:else if activeView === 'issues'}
        <IssuesView
          issues={actionableIssues}
          total={actionableIssueTotal}
          counts={actionableIssueCounts}
          {selectedIssueId}
          review={matchReview}
          loading={issuesLoading}
          loadingMore={issuesLoadingMore}
          reviewLoading={matchReviewLoading}
          decisionBusy={matchDecisionBusy}
          error={issueError}
          onSelect={selectIssue}
          onLoadMore={loadMoreIssues}
          onConfirm={confirmIssueMatch}
          onReject={rejectIssueMatch}
          onClearRejection={clearIssueRejection}
        />
      {:else}
        <div class="settings-page">
          <header class="page-header">
            <div>
              <h1 class="page-title">Settings</h1>
              <p class="page-description">
                Configure Refrain, manage integrations, and control how your
                music is synchronized.
              </p>
            </div>
          </header>
          <div class="settings-category-strip" aria-label="Settings categories">
            <span class="settings-category active"
              ><Icon name="settings" size={15} />General</span
            >
            <span class="settings-category"
              ><Icon name="sync" size={15} />Sync</span
            >
            <span class="settings-category"
              ><Icon name="spotify" size={15} />Spotify</span
            >
            <span class="settings-category"
              ><Icon name="local" size={15} />Library</span
            >
            <span class="settings-category"
              ><Icon name="download" size={15} />Acquisition</span
            >
            <span class="settings-category"
              ><Icon name="sliders" size={15} />Advanced</span
            >
          </div>
          <div class="settings-view">
            <div class="grid gap-3">
              <div class="rounded-lg border border-slate-800 p-4">
                <div class="flex items-start justify-between gap-4">
                  <div>
                    <p class="text-xs uppercase tracking-wider text-slate-600">
                      Spotify
                    </p>
                    <h2 class="mt-0.5 text-base font-semibold">Connection</h2>
                    <p class="mt-1.5 max-w-xl text-xs leading-5 text-slate-500">
                      The Client ID is saved locally. Refresh credentials are
                      stored in your operating system credential store.
                    </p>
                  </div>
                  {#if authStatus?.connected}
                    <span
                      class="rounded-full bg-emerald-950 px-3 py-1 text-xs text-emerald-300"
                    >
                      Connected
                    </span>
                  {:else}
                    <span
                      class={`rounded-full px-3 py-1 text-xs ${
                        soulseekConfigured
                          ? 'bg-emerald-950 text-emerald-300'
                          : 'bg-slate-900 text-slate-400'
                      }`}
                    >
                      Not connected
                    </span>
                  {/if}
                </div>

                <div class="mt-4 grid gap-1.5">
                  <label for="spotify-client-id" class="text-xs font-medium">
                    Spotify Client ID
                  </label>
                  <input
                    id="spotify-client-id"
                    bind:value={clientId}
                    disabled={authBusy || sourceBusy || authStatus?.connected}
                    autocomplete="off"
                    spellcheck="false"
                    placeholder="Paste your Spotify Client ID"
                    class="rounded-md border border-slate-700 bg-slate-900 px-2.5 py-2 font-mono text-xs outline-none transition focus:border-slate-500 disabled:cursor-not-allowed disabled:opacity-60"
                  />
                </div>

                <div
                  class="mt-4 rounded-md border border-slate-800 bg-slate-900/60 p-3"
                >
                  <p
                    class="text-xs font-medium uppercase tracking-wider text-slate-600"
                  >
                    Spotify redirect URI
                  </p>
                  <code class="mt-1.5 block break-all text-xs text-slate-200">
                    {authStatus?.registeredRedirectUri ?? fallbackRedirectUri}
                  </code>
                </div>

                {#if authError}
                  <div
                    class="mt-5 rounded-lg border border-amber-900 bg-amber-950/30 p-4 text-sm text-amber-200"
                  >
                    {authError}
                  </div>
                {/if}

                <div class="mt-4 flex flex-wrap items-center gap-2">
                  {#if authStatus?.connected}
                    <button
                      type="button"
                      onclick={disconnect}
                      disabled={authBusy || sourceBusy}
                      class="rounded-lg border border-slate-700 px-4 py-2 text-sm font-medium transition hover:bg-slate-900 disabled:cursor-not-allowed disabled:opacity-60"
                    >
                      {authBusy ? 'Disconnecting…' : 'Disconnect'}
                    </button>
                    <button
                      type="button"
                      onclick={refreshSource}
                      disabled={authBusy || sourceBusy}
                      class="rounded-lg bg-slate-100 px-4 py-2 text-sm font-medium text-slate-950 transition hover:bg-white disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      Refresh Spotify
                    </button>
                  {:else}
                    <button
                      type="button"
                      onclick={connect}
                      disabled={authBusy || !clientId.trim()}
                      class="rounded-lg bg-slate-100 px-4 py-2 text-sm font-medium text-slate-950 transition hover:bg-white disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      {authBusy ? 'Waiting for Spotify…' : 'Connect Spotify'}
                    </button>
                  {/if}
                </div>

                {#if sourceSummary}
                  <div
                    class="mt-6 rounded-lg border border-slate-800 bg-slate-900/60 p-4"
                  >
                    <p class="text-sm font-medium text-slate-200">
                      Spotify source refreshed
                    </p>
                    <p class="mt-2 text-xs leading-5 text-slate-500">
                      {sourceSummary.likedSongs} Liked Songs · {savedAlbumTotal} saved
                      albums ·
                      {sourceSummary.playlists} playlists ·
                      {sourceSummary.refreshedPlaylists} refreshed ·
                      {sourceSummary.unchangedPlaylists} unchanged
                      {#if sourceSummary.inaccessiblePlaylists > 0}
                        · {sourceSummary.inaccessiblePlaylists} inaccessible
                      {/if}
                    </p>
                  </div>
                {/if}
              </div>

              <div class="rounded-lg border border-slate-800 p-4">
                <p class="text-xs uppercase tracking-wider text-slate-600">
                  Local library
                </p>
                <h2 class="mt-0.5 text-base font-semibold">Library index</h2>
                <p class="mt-1.5 max-w-xl text-xs leading-5 text-slate-500">
                  Refrain scans this folder without moving, renaming, or
                  deleting your files.
                </p>

                <div class="mt-4 grid gap-1.5">
                  <label for="library-root" class="text-xs font-medium"
                    >Library root</label
                  >
                  <input
                    id="library-root"
                    bind:value={libraryRoot}
                    disabled={libraryBusy}
                    readonly
                    onclick={() => void pickLibraryRoot()}
                    onkeydown={(event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault();
                        void pickLibraryRoot();
                      }
                    }}
                    autocomplete="off"
                    spellcheck="false"
                    placeholder="Click to choose a music folder"
                    class="cursor-pointer rounded-md border border-slate-700 bg-slate-900 px-2.5 py-2 font-mono text-xs outline-none transition focus:border-slate-500 disabled:cursor-not-allowed disabled:opacity-60"
                  />
                </div>

                <div class="mt-4 flex flex-wrap items-center gap-2">
                  <button
                    type="button"
                    onclick={saveLibraryRoot}
                    disabled={libraryBusy}
                    class="rounded-lg border border-slate-700 px-4 py-2 text-sm font-medium transition hover:bg-slate-900 disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    Save path
                  </button>
                  <button
                    type="button"
                    onclick={scanLibraryRoot}
                    disabled={libraryBusy || !libraryRoot.trim()}
                    class="rounded-lg bg-slate-100 px-4 py-2 text-sm font-medium text-slate-950 transition hover:bg-white disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {libraryBusy ? 'Scanning…' : 'Scan Files'}
                  </button>
                </div>

                {#if libraryProgress}
                  <p class="mt-4 text-xs leading-5 text-slate-500">
                    {libraryProgress.message}
                  </p>
                {/if}

                {#if libraryOverview}
                  <p class="mt-4 text-xs leading-5 text-slate-500">
                    {libraryOverview.present} present · {libraryOverview.missing}
                    missing ·
                    {libraryOverview.invalid} invalid · {libraryOverview.total} indexed
                  </p>
                {/if}

                {#if librarySummary}
                  <p class="mt-2 text-xs leading-5 text-slate-600">
                    Last scan: {librarySummary.added} added · {librarySummary.updated}
                    updated ·
                    {librarySummary.moved} moved · {librarySummary.unchanged} unchanged
                  </p>
                {/if}

                {#if libraryError}
                  <div
                    class="mt-5 rounded-lg border border-amber-900 bg-amber-950/30 p-4 text-sm text-amber-200"
                  >
                    {libraryError}
                  </div>
                {/if}
              </div>

              <div class="rounded-lg border border-slate-800 p-4">
                <div class="flex items-start justify-between gap-4">
                  <div>
                    <p class="text-xs uppercase tracking-wider text-slate-600">
                      Acquisition
                    </p>
                    <h2 class="mt-0.5 text-base font-semibold">
                      Sockseek provider
                    </h2>
                    <p class="mt-1.5 max-w-xl text-xs leading-5 text-slate-500">
                      Sockseek is bundled with Refrain and connects to the
                      Soulseek network using your Soulseek account. There is no
                      separate Sockseek account.
                    </p>
                  </div>
                  <div class="flex flex-wrap justify-end gap-2">
                    <span
                      class={`rounded-full px-3 py-1 text-xs ${
                        soulseekConfigured
                          ? 'bg-emerald-950 text-emerald-300'
                          : 'bg-slate-900 text-slate-400'
                      }`}
                    >
                      {soulseekConfigured
                        ? 'Account configured'
                        : 'Account not configured'}
                    </span>
                    <span
                      class={`rounded-full px-3 py-1 text-xs ${
                        sockseekHealth?.available
                          ? 'bg-emerald-950 text-emerald-300'
                          : sockseekHealth
                            ? 'bg-amber-950 text-amber-300'
                            : 'bg-slate-900 text-slate-400'
                      }`}
                    >
                      {sockseekHealth?.available
                        ? 'Sockseek ready'
                        : sockseekHealth
                          ? 'Sockseek not ready'
                          : 'Sockseek not checked'}
                    </span>
                  </div>
                </div>

                <div
                  class="mt-4 rounded-md border border-slate-800 bg-slate-900/60 p-3 text-xs"
                >
                  <p class="font-medium text-slate-200">Setup</p>
                  <ol
                    class="mt-2 list-decimal space-y-1 pl-5 leading-6 text-slate-500"
                  >
                    <li>
                      Enter the same Soulseek username and password you use to
                      sign in to Soulseek, then save them.
                    </li>
                    <li>
                      Check Sockseek to start the bundled daemon and verify the
                      provider is ready.
                    </li>
                    <li>
                      Enable acquisition so missing tracks can be downloaded
                      during synchronization.
                    </li>
                  </ol>
                  <p class="mt-2 text-xs leading-5 text-slate-600">
                    Credentials are stored in your operating system credential
                    store. Refrain starts and configures Sockseek automatically.
                  </p>
                </div>

                <label class="mt-4 flex items-center gap-2.5 text-xs">
                  <input
                    type="checkbox"
                    bind:checked={acquisitionEnabled}
                    disabled={acquisitionBusy}
                    class="size-4 accent-slate-100"
                  />
                  Acquire missing tracks during synchronization
                </label>
                <p class="mt-2 max-w-xl text-xs leading-5 text-slate-500">
                  When enabled, Refrain searches Sockseek for tracks that are in
                  your Spotify source but still missing from your local library.
                  Downloads are staged for verification before they can become
                  canonical library files.
                </p>

                <div class="mt-4 grid gap-3 sm:grid-cols-2">
                  <div class="grid gap-1.5">
                    <label for="soulseek-username" class="text-xs font-medium">
                      Soulseek username
                    </label>
                    <input
                      id="soulseek-username"
                      bind:value={soulseekUsername}
                      disabled={acquisitionBusy}
                      autocomplete="username"
                      spellcheck="false"
                      class="rounded-md border border-slate-700 bg-slate-900 px-2.5 py-2 text-xs outline-none transition focus:border-slate-500 disabled:opacity-60"
                    />
                  </div>
                  <div class="grid gap-1.5">
                    <label for="soulseek-password" class="text-xs font-medium">
                      Soulseek password
                    </label>
                    <input
                      id="soulseek-password"
                      type="password"
                      bind:value={soulseekPassword}
                      disabled={acquisitionBusy}
                      autocomplete="current-password"
                      placeholder={soulseekConfigured
                        ? 'Saved in credential store'
                        : ''}
                      class="rounded-md border border-slate-700 bg-slate-900 px-2.5 py-2 text-xs outline-none transition focus:border-slate-500 disabled:opacity-60"
                    />
                  </div>
                </div>

                <div class="mt-4 flex flex-wrap items-center gap-2">
                  <button
                    type="button"
                    onclick={saveAcquisitionSettings}
                    disabled={acquisitionBusy}
                    class="rounded-lg border border-slate-700 px-4 py-2 text-sm font-medium transition hover:bg-slate-900 disabled:opacity-60"
                  >
                    Save acquisition setting
                  </button>
                  <button
                    type="button"
                    onclick={saveSoulseekAccount}
                    disabled={acquisitionBusy ||
                      !soulseekUsername.trim() ||
                      !soulseekPassword}
                    class="rounded-lg bg-slate-100 px-4 py-2 text-sm font-medium text-slate-950 transition hover:bg-white disabled:opacity-50"
                  >
                    Save Soulseek credentials
                  </button>
                  <button
                    type="button"
                    onclick={checkSockseekHealth}
                    disabled={acquisitionBusy || !soulseekConfigured}
                    class="rounded-lg border border-slate-700 px-4 py-2 text-sm font-medium transition hover:bg-slate-900 disabled:opacity-60"
                  >
                    Check Sockseek
                  </button>
                  {#if soulseekConfigured}
                    <button
                      type="button"
                      onclick={clearSoulseekAccount}
                      disabled={acquisitionBusy}
                      class="rounded-lg px-3 py-2 text-sm text-slate-400 transition hover:bg-slate-900 hover:text-slate-200 disabled:opacity-60"
                    >
                      Clear credentials
                    </button>
                  {/if}
                </div>

                {#if sockseekHealth}
                  <p class="mt-4 text-xs leading-5 text-slate-500">
                    Sockseek {sockseekHealth.version ?? 'unknown'} ·
                    {sockseekHealth.message ?? 'Provider ready'}
                  </p>
                {/if}

                {#if acquisitionError}
                  <div
                    class="mt-5 rounded-lg border border-amber-900 bg-amber-950/30 p-4 text-sm text-amber-200"
                  >
                    {acquisitionError}
                  </div>
                {/if}
              </div>
            </div>

            <aside class="rounded-lg border border-slate-800 p-4 text-xs">
              <h2 class="font-medium">Runtime</h2>
              {#if appInfo}
                <dl class="mt-4 grid gap-4">
                  <div>
                    <dt class="text-xs uppercase tracking-wider text-slate-600">
                      Application
                    </dt>
                    <dd class="mt-1">{appInfo.name} {appInfo.version}</dd>
                  </div>
                  <div>
                    <dt class="text-xs uppercase tracking-wider text-slate-600">
                      Data directory
                    </dt>
                    <dd
                      class="mt-1 break-all font-mono text-xs leading-5 text-slate-500"
                    >
                      {appInfo.dataDir}
                    </dd>
                  </div>
                  <div>
                    <dt class="text-xs uppercase tracking-wider text-slate-600">
                      Persisted source
                    </dt>
                    <dd class="mt-1 text-slate-400">
                      {sourceOverview?.likedSongs?.entryCount ?? 0} liked ·
                      {savedAlbumTotal} albums · {sourceOverview?.playlistCount ??
                        0}
                      playlists
                    </dd>
                  </div>
                </dl>
              {:else}
                <div
                  class="mt-4 h-20 animate-pulse rounded-lg bg-slate-900"
                ></div>
              {/if}
            </aside>
          </div>
        </div>
      {/if}
    </div>
  </section>
</main>
