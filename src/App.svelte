<script lang="ts">
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import IssuesView from './components/IssuesView.svelte';
  import LibraryView from './components/LibraryView.svelte';
  import TrackList from './components/TrackList.svelte';
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

  type AppView =
    'library' | 'liked' | 'albums' | 'playlists' | 'issues' | 'settings';

  const fallbackRedirectUri = 'http://127.0.0.1:43817/callback';
  const sourceProgressEvent = 'spotify-source-refresh-progress';
  const libraryProgressEvent = 'local-library-scan-progress';
  const collectionPageSize = 200;
  const collectionListPageSize = 100;
  const libraryTrackPageSize = 100;
  const issuePageSize = 100;

  let activeView: AppView = 'library';
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

  onMount(() => {
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
      if (!selectedSavedAlbum && savedAlbums.length > 0) {
        selectedSavedAlbum = savedAlbums[0];
      }

      if (selectedPlaylist) {
        selectedPlaylist =
          playlists.find((playlist) => playlist.id === selectedPlaylist?.id) ??
          null;
      }
      if (!selectedPlaylist && playlists.length > 0) {
        selectedPlaylist = playlists[0];
      }

      if (activeView === 'liked' && sourceOverview.likedSongs) {
        await loadCollection(sourceOverview.likedSongs);
      } else if (activeView === 'albums' && selectedSavedAlbum) {
        await loadCollection(selectedSavedAlbum);
      } else if (activeView === 'playlists' && selectedPlaylist) {
        await loadCollection(selectedPlaylist);
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

  async function openView(view: AppView) {
    activeView = view;
    collectionError = null;

    if (view === 'library') {
      await loadLibraryTracks();
    } else if (view === 'liked' && sourceOverview?.likedSongs) {
      await loadCollection(sourceOverview.likedSongs);
    } else if (view === 'albums' && selectedSavedAlbum) {
      await loadCollection(selectedSavedAlbum);
    } else if (view === 'playlists' && selectedPlaylist) {
      await loadCollection(selectedPlaylist);
    } else if (view === 'issues') {
      await loadIssues();
    }
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
    if (!value) {
      return 'Never refreshed';
    }
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short',
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
</script>

<svelte:head>
  <title>Refrain</title>
</svelte:head>

<main class="min-h-screen bg-slate-950 text-slate-100">
  <div class="mx-auto min-h-screen max-w-[92rem] px-6 py-6">
    <header
      class="flex items-center justify-between gap-6 border-b border-slate-800 pb-5"
    >
      <div>
        <div class="flex items-center gap-3">
          <h1 class="text-2xl font-semibold tracking-tight">Refrain</h1>
          <span
            class="rounded-full border border-slate-800 px-2.5 py-1 text-[11px] text-slate-400"
          >
            v0.1 desktop
          </span>
        </div>
        <p class="mt-1 text-sm text-slate-500">
          Spotify source state, persisted locally.
        </p>
      </div>

      <div class="flex items-center gap-3">
        {#if sourceOverview?.account}
          <div class="hidden text-right text-xs text-slate-500 sm:block">
            <p class="text-slate-300">
              {sourceOverview.account.displayName ?? 'Spotify account'}
            </p>
            <p>{formatSyncTime(sourceOverview.account.lastSourceSyncAt)}</p>
          </div>
        {/if}
        {#if authStatus?.connected}
          <button
            type="button"
            onclick={refreshSource}
            disabled={sourceBusy || authBusy}
            class="rounded-lg bg-slate-100 px-3.5 py-2 text-sm font-medium text-slate-950 transition hover:bg-white disabled:cursor-not-allowed disabled:opacity-50"
          >
            {sourceBusy ? 'Refreshing…' : 'Refresh Spotify'}
          </button>
        {:else}
          <button
            type="button"
            onclick={() => openView('settings')}
            class="rounded-lg border border-slate-700 px-3.5 py-2 text-sm font-medium text-slate-200 transition hover:bg-slate-900"
          >
            Connect Spotify
          </button>
        {/if}
      </div>
    </header>

    {#if sourceBusy && sourceProgress}
      <div
        class="mt-4 flex items-center justify-between gap-4 rounded-lg border border-slate-800 bg-slate-900/60 px-4 py-3 text-sm"
      >
        <div class="min-w-0">
          <p class="truncate text-slate-300">{sourceProgress.message}</p>
          {#if sourceProgress.total !== null}
            <p class="mt-0.5 text-xs text-slate-600">
              {sourceProgress.completed} / {sourceProgress.total}
            </p>
          {/if}
        </div>
        <button
          type="button"
          onclick={cancelSourceRefresh}
          class="shrink-0 rounded-md border border-slate-700 px-3 py-1.5 text-xs font-medium text-slate-300 hover:bg-slate-800"
        >
          Cancel
        </button>
      </div>
    {:else if sourceError}
      <div
        class="mt-4 rounded-lg border border-amber-900 bg-amber-950/30 px-4 py-3 text-sm text-amber-200"
      >
        {sourceError}
      </div>
    {/if}

    <div class="grid gap-6 py-6 lg:grid-cols-[13rem_minmax(0,1fr)]">
      <nav class="space-y-1" aria-label="Primary">
        <button
          type="button"
          onclick={() => openView('library')}
          class={`flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-sm transition ${activeView === 'library' ? 'bg-slate-900 text-white' : 'text-slate-400 hover:bg-slate-900/60 hover:text-slate-200'}`}
        >
          <span>Library</span>
          <span class="font-mono text-xs text-slate-600"
            >{libraryTrackTotal}</span
          >
        </button>
        <button
          type="button"
          onclick={() => openView('liked')}
          class={`flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-sm transition ${activeView === 'liked' ? 'bg-slate-900 text-white' : 'text-slate-400 hover:bg-slate-900/60 hover:text-slate-200'}`}
        >
          <span>Liked Songs</span>
          {#if sourceOverview?.likedSongs}
            <span class="font-mono text-xs text-slate-600">
              {sourceOverview.likedSongs.entryCount}
            </span>
          {/if}
        </button>
        <button
          type="button"
          onclick={() => openView('albums')}
          class={`flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-sm transition ${activeView === 'albums' ? 'bg-slate-900 text-white' : 'text-slate-400 hover:bg-slate-900/60 hover:text-slate-200'}`}
        >
          <span>Saved Albums</span>
          <span class="font-mono text-xs text-slate-600">{savedAlbumTotal}</span
          >
        </button>
        <button
          type="button"
          onclick={() => openView('playlists')}
          class={`flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-sm transition ${activeView === 'playlists' ? 'bg-slate-900 text-white' : 'text-slate-400 hover:bg-slate-900/60 hover:text-slate-200'}`}
        >
          <span>Playlists</span>
          {#if sourceOverview}
            <span class="font-mono text-xs text-slate-600">
              {sourceOverview.playlistCount}
            </span>
          {/if}
        </button>
        <button
          type="button"
          onclick={() => openView('issues')}
          class={`flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-sm transition ${activeView === 'issues' ? 'bg-slate-900 text-white' : 'text-slate-400 hover:bg-slate-900/60 hover:text-slate-200'}`}
        >
          <span>Issues</span>
          {#if issueTotal > 0}
            <span
              class="rounded bg-amber-950 px-1.5 py-0.5 font-mono text-[10px] text-amber-300"
            >
              {issueTotal}
            </span>
          {:else}
            <span class="font-mono text-xs text-slate-700">0</span>
          {/if}
        </button>
        <button
          type="button"
          onclick={() => openView('settings')}
          class={`w-full rounded-lg px-3 py-2 text-left text-sm transition ${activeView === 'settings' ? 'bg-slate-900 text-white' : 'text-slate-400 hover:bg-slate-900/60 hover:text-slate-200'}`}
        >
          Settings
        </button>

        <div class="pt-5">
          <div class="border-t border-slate-900 pt-4 text-xs text-slate-600">
            {#if authStatus?.connected}
              <p class="text-emerald-500">Spotify connected</p>
            {:else}
              <p>Spotify disconnected</p>
            {/if}
            {#if sourceSummary}
              <p class="mt-2 leading-5">
                Last refresh: {sourceSummary.likedSongs} liked · {savedAlbumTotal}
                albums · {sourceSummary.playlists} playlists
              </p>
            {/if}
          </div>
        </div>
      </nav>

      <section class="min-w-0">
        {#if backendError}
          <div
            class="rounded-xl border border-amber-900 bg-amber-950/30 p-5 text-sm text-amber-200"
          >
            {backendError}
          </div>
        {:else if sourceHydrating && !sourceOverview}
          <div class="grid gap-3">
            <div class="h-16 animate-pulse rounded-xl bg-slate-900"></div>
            <div class="h-[34rem] animate-pulse rounded-xl bg-slate-900"></div>
          </div>
        {:else if sourceHydrationError}
          <div
            class="rounded-xl border border-amber-900 bg-amber-950/30 p-5 text-sm text-amber-200"
          >
            <p class="font-medium">Could not load persisted Spotify state.</p>
            <p class="mt-1 text-amber-300/80">{sourceHydrationError}</p>
          </div>
        {:else if activeView === 'library'}
          <LibraryView
            tracks={libraryTracks}
            total={libraryTrackTotal}
            loading={libraryTracksLoading}
            loadingMore={libraryTracksLoadingMore}
            error={libraryTracksError}
            onLoadMore={loadMoreLibraryTracks}
          />
        {:else if activeView === 'liked'}
          <div>
            <div class="mb-5 flex items-end justify-between gap-4">
              <div>
                <p class="text-xs uppercase tracking-wider text-slate-600">
                  Collection
                </p>
                <h2 class="mt-1 text-2xl font-semibold">Liked Songs</h2>
                <p class="mt-1 text-sm text-slate-500">
                  {sourceOverview?.likedSongs?.entryCount ?? 0} imported entries
                </p>
              </div>
            </div>

            {#if !sourceOverview?.likedSongs}
              <div
                class="rounded-xl border border-dashed border-slate-800 px-6 py-14 text-center"
              >
                <p class="text-sm font-medium text-slate-300">
                  No imported Liked Songs yet.
                </p>
                <p class="mt-2 text-sm text-slate-500">
                  {authStatus?.connected
                    ? 'Refresh Spotify to import your source state.'
                    : 'Connect Spotify in Settings, then run a refresh.'}
                </p>
              </div>
            {:else if collectionError}
              <div
                class="rounded-xl border border-amber-900 bg-amber-950/30 p-5 text-sm text-amber-200"
              >
                {collectionError}
              </div>
            {:else}
              <TrackList
                entries={collectionEntries}
                total={collectionTotal}
                loading={collectionLoading}
                loadingMore={collectionLoadingMore}
                emptyMessage="No Liked Songs are currently imported."
                onLoadMore={loadMoreCollection}
              />
            {/if}
          </div>
        {:else if activeView === 'albums'}
          <div class="grid min-w-0 gap-5 xl:grid-cols-[18rem_minmax(0,1fr)]">
            <aside
              class="overflow-hidden rounded-xl border border-slate-800 bg-slate-950/40"
            >
              <div class="border-b border-slate-800 px-4 py-3">
                <h2 class="font-medium">Saved Albums</h2>
                <p class="mt-0.5 text-xs text-slate-600">
                  {savedAlbumTotal} imported
                </p>
              </div>

              {#if savedAlbums.length === 0}
                <p class="px-4 py-8 text-sm text-slate-500">
                  No saved albums imported yet.
                </p>
              {:else}
                <div class="max-h-[38rem] overflow-y-auto p-2">
                  {#each savedAlbums as album (album.id)}
                    <button
                      type="button"
                      onclick={() => selectSavedAlbum(album)}
                      class={`mb-1 flex w-full items-start justify-between gap-3 rounded-lg px-3 py-2.5 text-left transition ${selectedSavedAlbum?.id === album.id ? 'bg-slate-900' : 'hover:bg-slate-900/60'}`}
                    >
                      <span class="min-w-0">
                        <span class="block truncate text-sm text-slate-200">
                          {album.name}
                        </span>
                        <span class="mt-0.5 block text-xs text-slate-600">
                          {album.entryCount} tracks
                        </span>
                      </span>
                    </button>
                  {/each}

                  {#if savedAlbums.length < savedAlbumTotal}
                    <button
                      type="button"
                      onclick={loadMoreSavedAlbums}
                      disabled={savedAlbumsLoadingMore}
                      class="mt-2 w-full rounded-lg border border-slate-800 px-3 py-2 text-xs font-medium text-slate-400 hover:bg-slate-900 disabled:opacity-50"
                    >
                      {savedAlbumsLoadingMore ? 'Loading…' : 'Load more albums'}
                    </button>
                  {/if}
                </div>
              {/if}
            </aside>

            <div class="min-w-0">
              {#if !selectedSavedAlbum}
                <div
                  class="rounded-xl border border-dashed border-slate-800 px-6 py-14 text-center text-sm text-slate-500"
                >
                  Select a saved album to inspect its imported tracks.
                </div>
              {:else}
                <div class="mb-5">
                  <p class="text-xs uppercase tracking-wider text-slate-600">
                    Saved Album
                  </p>
                  <h2 class="mt-1 truncate text-2xl font-semibold">
                    {selectedSavedAlbum.name}
                  </h2>
                  <p class="mt-1 text-sm text-slate-500">
                    {selectedSavedAlbum.entryCount} imported tracks
                  </p>
                </div>

                {#if collectionError}
                  <div
                    class="rounded-xl border border-amber-900 bg-amber-950/30 p-5 text-sm text-amber-200"
                  >
                    {collectionError}
                  </div>
                {:else}
                  <TrackList
                    entries={collectionEntries}
                    total={collectionTotal}
                    loading={collectionLoading}
                    loadingMore={collectionLoadingMore}
                    emptyMessage="This saved album has no imported tracks."
                    onLoadMore={loadMoreCollection}
                  />
                {/if}
              {/if}
            </div>
          </div>
        {:else if activeView === 'playlists'}
          <div class="grid min-w-0 gap-5 xl:grid-cols-[18rem_minmax(0,1fr)]">
            <aside
              class="overflow-hidden rounded-xl border border-slate-800 bg-slate-950/40"
            >
              <div class="border-b border-slate-800 px-4 py-3">
                <h2 class="font-medium">Playlists</h2>
                <p class="mt-0.5 text-xs text-slate-600">
                  {playlistTotal} imported
                </p>
              </div>

              {#if playlists.length === 0}
                <p class="px-4 py-8 text-sm text-slate-500">
                  No playlists imported yet.
                </p>
              {:else}
                <div class="max-h-[38rem] overflow-y-auto p-2">
                  {#each playlists as playlist (playlist.id)}
                    <button
                      type="button"
                      onclick={() => selectPlaylist(playlist)}
                      class={`mb-1 flex w-full items-start justify-between gap-3 rounded-lg px-3 py-2.5 text-left transition ${selectedPlaylist?.id === playlist.id ? 'bg-slate-900' : 'hover:bg-slate-900/60'}`}
                    >
                      <span class="min-w-0">
                        <span class="block truncate text-sm text-slate-200">
                          {playlist.name}
                        </span>
                        <span class="mt-0.5 block text-xs text-slate-600">
                          {playlist.entryCount} entries
                        </span>
                      </span>
                      {#if !playlist.isAccessible}
                        <span
                          class="mt-0.5 shrink-0 rounded bg-amber-950 px-1.5 py-0.5 text-[10px] uppercase text-amber-300"
                        >
                          inaccessible
                        </span>
                      {/if}
                    </button>
                  {/each}

                  {#if playlists.length < playlistTotal}
                    <button
                      type="button"
                      onclick={loadMorePlaylists}
                      disabled={playlistsLoadingMore}
                      class="mt-2 w-full rounded-lg border border-slate-800 px-3 py-2 text-xs font-medium text-slate-400 hover:bg-slate-900 disabled:opacity-50"
                    >
                      {playlistsLoadingMore
                        ? 'Loading…'
                        : 'Load more playlists'}
                    </button>
                  {/if}
                </div>
              {/if}
            </aside>

            <div class="min-w-0">
              {#if !selectedPlaylist}
                <div
                  class="rounded-xl border border-dashed border-slate-800 px-6 py-14 text-center text-sm text-slate-500"
                >
                  Select a playlist to inspect its imported entries.
                </div>
              {:else}
                <div class="mb-5">
                  <p class="text-xs uppercase tracking-wider text-slate-600">
                    Playlist
                  </p>
                  <h2 class="mt-1 truncate text-2xl font-semibold">
                    {selectedPlaylist.name}
                  </h2>
                  <p class="mt-1 text-sm text-slate-500">
                    {selectedPlaylist.entryCount} imported entries
                  </p>
                </div>

                {#if !selectedPlaylist.isAccessible}
                  <div
                    class="rounded-xl border border-amber-900 bg-amber-950/20 p-6"
                  >
                    <h3 class="font-medium text-amber-200">
                      Playlist is inaccessible
                    </h3>
                    <p class="mt-2 text-sm leading-6 text-amber-300/70">
                      {selectedPlaylist.accessIssue ??
                        'Spotify did not allow Refrain to read this playlist. Previously imported entries, if any, are preserved.'}
                    </p>
                  </div>
                {:else if collectionError}
                  <div
                    class="rounded-xl border border-amber-900 bg-amber-950/30 p-5 text-sm text-amber-200"
                  >
                    {collectionError}
                  </div>
                {:else}
                  <TrackList
                    entries={collectionEntries}
                    total={collectionTotal}
                    loading={collectionLoading}
                    loadingMore={collectionLoadingMore}
                    emptyMessage="This playlist has no imported entries."
                    onLoadMore={loadMoreCollection}
                  />
                {/if}
              {/if}
            </div>
          </div>
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
            error={issueError}
            onSelect={selectIssue}
            onLoadMore={loadMoreIssues}
            onConfirm={confirmIssueMatch}
            onReject={rejectIssueMatch}
            onClearRejection={clearIssueRejection}
          />
        {:else}
          <div class="grid gap-5 xl:grid-cols-[minmax(0,1fr)_20rem]">
            <div class="grid gap-5">
              <div class="rounded-xl border border-slate-800 p-6">
                <div class="flex items-start justify-between gap-4">
                  <div>
                    <p class="text-xs uppercase tracking-wider text-slate-600">
                      Spotify
                    </p>
                    <h2 class="mt-1 text-xl font-semibold">Connection</h2>
                    <p class="mt-2 max-w-xl text-sm leading-6 text-slate-500">
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
                      class="rounded-full bg-slate-900 px-3 py-1 text-xs text-slate-400"
                    >
                      Not connected
                    </span>
                  {/if}
                </div>

                <div class="mt-6 grid gap-2">
                  <label for="spotify-client-id" class="text-sm font-medium">
                    Spotify Client ID
                  </label>
                  <input
                    id="spotify-client-id"
                    bind:value={clientId}
                    disabled={authBusy || sourceBusy || authStatus?.connected}
                    autocomplete="off"
                    spellcheck="false"
                    placeholder="Paste your Spotify Client ID"
                    class="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2.5 font-mono text-sm outline-none transition focus:border-slate-500 disabled:cursor-not-allowed disabled:opacity-60"
                  />
                </div>

                <div
                  class="mt-5 rounded-lg border border-slate-800 bg-slate-900/60 p-4"
                >
                  <p
                    class="text-xs font-medium uppercase tracking-wider text-slate-600"
                  >
                    Spotify redirect URI
                  </p>
                  <code class="mt-2 block break-all text-sm text-slate-200">
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

                <div class="mt-6 flex flex-wrap items-center gap-3">
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

              <div class="rounded-xl border border-slate-800 p-6">
                <p class="text-xs uppercase tracking-wider text-slate-600">
                  Local library
                </p>
                <h2 class="mt-1 text-xl font-semibold">Library index</h2>
                <p class="mt-2 max-w-xl text-sm leading-6 text-slate-500">
                  Refrain scans this folder without moving, renaming, or
                  deleting your files.
                </p>

                <div class="mt-6 grid gap-2">
                  <label for="library-root" class="text-sm font-medium"
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
                    class="cursor-pointer rounded-lg border border-slate-700 bg-slate-900 px-3 py-2.5 font-mono text-sm outline-none transition focus:border-slate-500 disabled:cursor-not-allowed disabled:opacity-60"
                  />
                </div>

                <div class="mt-5 flex flex-wrap items-center gap-3">
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
                    {libraryBusy ? 'Scanning…' : 'Scan library'}
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

              <div class="rounded-xl border border-slate-800 p-6">
                <div class="flex items-start justify-between gap-4">
                  <div>
                    <p class="text-xs uppercase tracking-wider text-slate-600">
                      Acquisition
                    </p>
                    <h2 class="mt-1 text-xl font-semibold">
                      Sockseek provider
                    </h2>
                    <p class="mt-2 max-w-xl text-sm leading-6 text-slate-500">
                      Sockseek is bundled with Refrain and connects to the
                      Soulseek network using your Soulseek account. There is no
                      separate Sockseek account.
                    </p>
                  </div>
                  <span
                    class="rounded-full bg-slate-900 px-3 py-1 text-xs text-slate-400"
                  >
                    {soulseekConfigured
                      ? 'Account configured'
                      : 'Not configured'}
                  </span>
                </div>

                <div
                  class="mt-5 rounded-lg border border-slate-800 bg-slate-900/60 p-4 text-sm"
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
                      Check the provider to start Sockseek and verify the
                      Soulseek login.
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

                <label class="mt-6 flex items-center gap-3 text-sm">
                  <input
                    type="checkbox"
                    bind:checked={acquisitionEnabled}
                    disabled={acquisitionBusy}
                    class="size-4 accent-slate-100"
                  />
                  Acquire missing tracks during synchronization
                </label>

                <div class="mt-5 grid gap-4 sm:grid-cols-2">
                  <div class="grid gap-2">
                    <label for="soulseek-username" class="text-sm font-medium">
                      Soulseek username
                    </label>
                    <input
                      id="soulseek-username"
                      bind:value={soulseekUsername}
                      disabled={acquisitionBusy}
                      autocomplete="username"
                      spellcheck="false"
                      class="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2.5 text-sm outline-none transition focus:border-slate-500 disabled:opacity-60"
                    />
                  </div>
                  <div class="grid gap-2">
                    <label for="soulseek-password" class="text-sm font-medium">
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
                      class="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2.5 text-sm outline-none transition focus:border-slate-500 disabled:opacity-60"
                    />
                  </div>
                </div>

                <div class="mt-5 flex flex-wrap items-center gap-3">
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
                    Check Sockseek connection
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
                    {sockseekHealth.available
                      ? 'Soulseek ready'
                      : (sockseekHealth.message ?? 'Unavailable')}
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

            <aside class="rounded-xl border border-slate-800 p-5 text-sm">
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
        {/if}
      </section>
    </div>
  </div>
</main>
