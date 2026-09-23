<script lang="ts">
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import TrackList from './components/TrackList.svelte';
  import { getAppInfo, type AppInfo } from './lib/app-info';
  import {
    getSourceCollectionPage,
    hydrateSpotifySource,
    listSpotifyPlaylists,
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

  type AppView = 'liked' | 'playlists' | 'settings';

  const fallbackRedirectUri = 'http://127.0.0.1:43817/callback';
  const sourceProgressEvent = 'spotify-source-refresh-progress';
  const collectionPageSize = 200;
  const playlistPageSize = 100;

  let activeView: AppView = 'liked';
  let appInfo: AppInfo | null = null;
  let authStatus: SpotifyAuthStatus | null = null;
  let clientId = '';
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

  onMount(() => {
    let disposed = false;
    let unlisten: UnlistenFn | undefined;

    void listen<SpotifySourceRefreshProgress>(sourceProgressEvent, (event) => {
      sourceProgress = event.payload;
    }).then((stopListening) => {
      if (disposed) {
        stopListening();
      } else {
        unlisten = stopListening;
      }
    });
    void initialize();

    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  async function initialize() {
    try {
      const [info, status] = await Promise.all([
        getAppInfo(),
        getSpotifyAuthStatus(),
      ]);
      appInfo = info;
      authStatus = status;
      clientId = status.clientId ?? '';
      backendError = null;
      await hydrateSourceState();
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
      playlists = hydrated.playlists.items;
      playlistTotal = hydrated.playlists.total;

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

  async function openView(view: AppView) {
    activeView = view;
    collectionError = null;

    if (view === 'liked' && sourceOverview?.likedSongs) {
      await loadCollection(sourceOverview.likedSongs);
    } else if (view === 'playlists' && selectedPlaylist) {
      await loadCollection(selectedPlaylist);
    }
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

  async function loadMorePlaylists() {
    if (playlistsLoadingMore || playlists.length >= playlistTotal) {
      return;
    }
    playlistsLoadingMore = true;
    sourceHydrationError = null;
    try {
      const page = await listSpotifyPlaylists(
        playlists.length,
        playlistPageSize,
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
                Last refresh: {sourceSummary.likedSongs} liked ·
                {sourceSummary.playlists} playlists
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
        {:else}
          <div class="grid gap-5 xl:grid-cols-[minmax(0,1fr)_20rem]">
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
                    {sourceSummary.likedSongs} Liked Songs ·
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
                      {sourceOverview?.playlistCount ?? 0} playlists
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
