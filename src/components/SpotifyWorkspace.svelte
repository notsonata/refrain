<script lang="ts">
  import TrackList from './TrackList.svelte';
  import type { SyncRun } from '../lib/sync';
  import type {
    SourceCollectionEntryView,
    SourceCollectionSummary,
    SpotifySourceOverview,
  } from '../lib/source';

  type SpotifySection = 'liked' | 'albums' | 'playlists';

  export let section: SpotifySection = 'liked';
  export let overview: SpotifySourceOverview | null = null;
  export let savedAlbums: SourceCollectionSummary[] = [];
  export let savedAlbumTotal = 0;
  export let playlists: SourceCollectionSummary[] = [];
  export let playlistTotal = 0;
  export let currentCollection: SourceCollectionSummary | null = null;
  export let entries: SourceCollectionEntryView[] = [];
  export let collectionTotal = 0;
  export let collectionLoading = false;
  export let collectionLoadingMore = false;
  export let collectionError: string | null = null;
  export let savedAlbumsLoadingMore = false;
  export let playlistsLoadingMore = false;
  export let sourceBusy = false;
  export let syncBusy = false;
  export let syncRun: SyncRun | null = null;
  export let trackingBusyId: number | null = null;

  export let onSectionChange: ((section: SpotifySection) => void) | undefined = undefined;
  export let onSelectSavedAlbum: ((album: SourceCollectionSummary) => void) | undefined = undefined;
  export let onSelectPlaylist: ((playlist: SourceCollectionSummary) => void) | undefined = undefined;
  export let onBackToCollections: (() => void) | undefined = undefined;
  export let onLoadMoreSavedAlbums: (() => void) | undefined = undefined;
  export let onLoadMorePlaylists: (() => void) | undefined = undefined;
  export let onLoadMoreCollection: (() => void) | undefined = undefined;
  export let onSetCollectionTracking:
    | ((collection: SourceCollectionSummary, included: boolean) => void)
    | undefined = undefined;
  export let onSetTrackTracking:
    | ((entry: SourceCollectionEntryView, included: boolean | null) => void)
    | undefined = undefined;
  export let onSynchronize: (() => void) | undefined = undefined;
  export let onRefresh: (() => void) | undefined = undefined;

  let collectionSearch = '';
  let collectionTracking = 'all';

  $: accessiblePlaylists = playlists.filter((playlist) => playlist.isAccessible);
  $: unavailablePlaylists = playlists.filter((playlist) => !playlist.isAccessible);
  $: collectionList = section === 'albums' ? savedAlbums : accessiblePlaylists;
  $: filteredCollections = collectionList.filter((collection) => {
    const query = collectionSearch.trim().toLocaleLowerCase();
    if (query && !collection.name.toLocaleLowerCase().includes(query)) return false;
    if (collectionTracking === 'tracked' && collection.trackedEntryCount <= 0) return false;
    if (collectionTracking === 'partial' && !(collection.trackedEntryCount > 0 && collection.trackedEntryCount < collection.entryCount)) return false;
    if (collectionTracking === 'untracked' && collection.trackedEntryCount > 0) return false;
    return true;
  });
  $: displayedCollection =
    section === 'liked'
      ? currentCollection?.id === overview?.likedSongs?.id
        ? currentCollection
        : overview?.likedSongs ?? null
      : currentCollection;

  function collectionStatus(collection: SourceCollectionSummary): string {
    if (collection.trackedEntryCount <= 0) return 'Not tracked';
    if (collection.trackedEntryCount >= collection.entryCount) return 'Tracked';
    return `${collection.trackedEntryCount}/${collection.entryCount} tracked`;
  }

  function statusClass(collection: SourceCollectionSummary): string {
    if (collection.trackedEntryCount <= 0) return 'border-slate-800 text-slate-600';
    if (collection.trackedEntryCount >= collection.entryCount) {
      return 'border-emerald-900/70 bg-emerald-950/35 text-emerald-300';
    }
    return 'border-sky-900/70 bg-sky-950/30 text-sky-300';
  }
</script>

<div class="flex h-full min-h-0 flex-col">
  <div class="mb-3 flex shrink-0 items-start justify-between gap-4">
    <div class="min-w-0">
      <p class="text-[10px] font-medium uppercase tracking-[0.14em] text-slate-600">Spotify</p>
      <h2 class="mt-0.5 text-lg font-semibold tracking-tight">Choose what Refrain follows</h2>
      <p class="mt-0.5 max-w-2xl text-xs text-slate-500">
        Tracking persists across refreshes. Spotify Sync only reconciles what you include.
      </p>
    </div>
    <div class="flex shrink-0 items-center gap-1.5">
      <button
        type="button"
        onclick={() => onRefresh?.()}
        disabled={sourceBusy || syncBusy}
        class="rounded-md border border-slate-800 px-2.5 py-1.5 text-[11px] font-medium text-slate-400 hover:bg-slate-900 hover:text-slate-200 disabled:opacity-50"
      >
        {sourceBusy ? 'Refreshing…' : 'Refresh'}
      </button>
      <button
        type="button"
        onclick={() => onSynchronize?.()}
        disabled={syncBusy || sourceBusy}
        class="rounded-md bg-emerald-400 px-3 py-1.5 text-xs font-semibold text-emerald-950 hover:bg-emerald-300 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {syncBusy ? 'Syncing…' : 'Sync tracked'}
      </button>
    </div>
  </div>

  <div class="mb-2.5 flex shrink-0 items-center justify-between gap-3 border-b border-slate-900 pb-2.5">
    <div class="inline-flex rounded-md border border-slate-800 bg-slate-950 p-0.5">
      {#each [['liked', 'Liked Songs'], ['albums', 'Albums'], ['playlists', 'Playlists']] as option (option[0])}
        <button
          type="button"
          onclick={() => {
            collectionSearch = '';
            collectionTracking = 'all';
            onSectionChange?.(option[0] as SpotifySection);
          }}
          class={`rounded px-2.5 py-1 text-[11px] font-medium transition ${section === option[0] ? 'bg-slate-800 text-slate-100' : 'text-slate-500 hover:text-slate-300'}`}
        >
          {option[1]}
        </button>
      {/each}
    </div>
    <div class="text-right text-[10px] text-slate-600">
      {#if syncRun}
        Last sync: <span class="text-slate-400">{syncRun.status}</span>
        <span class="text-slate-800"> · </span>{syncRun.matched} matched
        <span class="text-slate-800"> · </span>{syncRun.missing} missing
      {:else}
        No Spotify sync yet
      {/if}
    </div>
  </div>

  {#if section === 'liked'}
    <div class="flex min-h-0 flex-1 flex-col">
      {#if !overview?.likedSongs}
        <div class="flex min-h-0 flex-1 items-center justify-center rounded-lg border border-dashed border-slate-800 px-5 text-center text-xs text-slate-500">
          Refresh Spotify to load Liked Songs.
        </div>
      {:else}
        <div class="mb-2.5 flex shrink-0 items-center justify-between rounded-lg border border-slate-800/80 bg-slate-900/25 px-3 py-2">
          <div class="min-w-0">
            <div class="flex items-center gap-1.5">
              <h3 class="text-xs font-medium text-slate-200">Liked Songs</h3>
              <span class={`rounded-full border px-1.5 py-0.5 text-[8px] font-medium ${statusClass(displayedCollection ?? overview.likedSongs)}`}>
                {collectionStatus(displayedCollection ?? overview.likedSongs)}
              </span>
            </div>
            <p class="mt-0.5 text-[10px] text-slate-600">
              {overview.likedSongs.entryCount} songs · individual overrides persist
            </p>
          </div>
          <button
            type="button"
            onclick={() => onSetCollectionTracking?.(
              displayedCollection ?? overview!.likedSongs!,
              !(displayedCollection ?? overview!.likedSongs!).trackedByDefault,
            )}
            disabled={trackingBusyId === -overview.likedSongs.id}
            class={`rounded-md border px-2.5 py-1.5 text-[10px] font-semibold transition disabled:opacity-40 ${(displayedCollection ?? overview.likedSongs).trackedByDefault ? 'border-emerald-900/70 bg-emerald-950/30 text-emerald-300' : 'border-slate-700 text-slate-300 hover:bg-slate-900'}`}
          >
            {(displayedCollection ?? overview.likedSongs).trackedByDefault ? 'Tracking by default' : 'Track all by default'}
          </button>
        </div>
        {#if collectionError}
          <div class="mb-2 rounded-md border border-amber-900 bg-amber-950/25 px-3 py-2 text-xs text-amber-200">{collectionError}</div>
        {/if}
        <TrackList
          {entries}
          total={collectionTotal}
          loading={collectionLoading}
          loadingMore={collectionLoadingMore}
          trackingControls={true}
          {trackingBusyId}
          emptyMessage="No Liked Songs are currently imported."
          onLoadMore={onLoadMoreCollection}
          onSetTracking={onSetTrackTracking}
        />
      {/if}
    </div>
  {:else if displayedCollection}
    <div class="flex min-h-0 flex-1 flex-col">
      <div class="mb-2.5 flex shrink-0 items-start justify-between gap-3">
        <div class="flex min-w-0 items-center gap-2.5">
          <button
            type="button"
            onclick={() => onBackToCollections?.()}
            class="shrink-0 rounded-md border border-slate-800 px-2 py-1 text-[10px] font-medium text-slate-500 hover:bg-slate-900 hover:text-slate-200"
          >
            ← {section === 'albums' ? 'Albums' : 'Playlists'}
          </button>
          {#if displayedCollection.imageUrl}
            <img src={displayedCollection.imageUrl} alt="" loading="lazy" class="h-10 w-10 shrink-0 rounded-md bg-slate-900 object-cover" />
          {/if}
          <div class="min-w-0">
            <div class="flex min-w-0 items-center gap-1.5">
              <h3 class="truncate text-base font-semibold text-slate-200">{displayedCollection.name}</h3>
              <span class={`shrink-0 rounded-full border px-1.5 py-0.5 text-[8px] font-medium ${statusClass(displayedCollection)}`}>
                {collectionStatus(displayedCollection)}
              </span>
            </div>
            <p class="mt-0.5 text-[10px] text-slate-600">{displayedCollection.entryCount} tracks · overrides persist across refreshes</p>
          </div>
        </div>
        <button
          type="button"
          onclick={() => onSetCollectionTracking?.(displayedCollection!, !displayedCollection!.trackedByDefault)}
          disabled={trackingBusyId === -displayedCollection.id}
          class={`shrink-0 rounded-md border px-2.5 py-1.5 text-[10px] font-semibold transition disabled:opacity-40 ${displayedCollection.trackedByDefault ? 'border-emerald-900/70 bg-emerald-950/30 text-emerald-300' : 'border-slate-700 text-slate-300 hover:bg-slate-900'}`}
        >
          {displayedCollection.trackedByDefault ? 'Tracking by default' : 'Track all by default'}
        </button>
      </div>
      {#if collectionError}
        <div class="mb-2 rounded-md border border-amber-900 bg-amber-950/25 px-3 py-2 text-xs text-amber-200">{collectionError}</div>
      {/if}
      <TrackList
        {entries}
        total={collectionTotal}
        loading={collectionLoading}
        loadingMore={collectionLoadingMore}
        trackingControls={true}
        {trackingBusyId}
        emptyMessage="This collection has no imported tracks."
        onLoadMore={onLoadMoreCollection}
        onSetTracking={onSetTrackTracking}
      />
    </div>
  {:else}
    <div class="flex min-h-0 flex-1 flex-col">
      <div class="mb-2.5 flex shrink-0 items-center gap-2">
        <input
          bind:value={collectionSearch}
          aria-label={`Search ${section}`}
          placeholder={`Search ${section === 'albums' ? 'albums' : 'playlists'}…`}
          class="min-w-0 flex-1 rounded-md border border-slate-800 bg-slate-900/70 px-2.5 py-1.5 text-xs text-slate-200 outline-none placeholder:text-slate-600 focus:border-slate-600"
        />
        <select
          bind:value={collectionTracking}
          aria-label="Collection tracking filter"
          class="rounded-md border border-slate-800 bg-slate-900/70 px-2.5 py-1.5 pr-7 text-[11px] text-slate-400 outline-none"
        >
          <option value="all">Tracking: all</option>
          <option value="tracked">Tracked</option>
          <option value="partial">Partial</option>
          <option value="untracked">Untracked</option>
        </select>
        <span class="shrink-0 text-[10px] text-slate-600">{filteredCollections.length} shown</span>
      </div>

      <div class="min-h-0 flex-1 overflow-y-auto pr-1">
        {#if collectionList.length === 0}
          <div class="flex min-h-[14rem] items-center justify-center rounded-lg border border-dashed border-slate-800 text-xs text-slate-500">
            No {section === 'albums' ? 'saved albums' : 'playlists'} imported yet.
          </div>
        {:else if filteredCollections.length === 0}
          <div class="flex min-h-[14rem] items-center justify-center rounded-lg border border-dashed border-slate-800 text-xs text-slate-500">
            No collections match the current filters.
          </div>
        {:else}
          <div class="grid grid-cols-2 gap-2.5 sm:grid-cols-3 lg:grid-cols-4 2xl:grid-cols-6">
            {#each filteredCollections as collection (collection.id)}
              <button
                type="button"
                onclick={() => section === 'albums' ? onSelectSavedAlbum?.(collection) : onSelectPlaylist?.(collection)}
                class="group min-w-0 rounded-lg border border-slate-800/80 bg-slate-950/40 p-2 text-left transition hover:border-slate-700 hover:bg-slate-900/55"
              >
                <div class="aspect-square overflow-hidden rounded-md bg-slate-900">
                  {#if collection.imageUrl}
                    <img src={collection.imageUrl} alt="" loading="lazy" class="h-full w-full object-cover transition group-hover:scale-[1.02]" />
                  {:else}
                    <div class="flex h-full w-full items-center justify-center text-lg font-semibold text-slate-700">
                      {collection.name.slice(0, 1).toUpperCase()}
                    </div>
                  {/if}
                </div>
                <div class="mt-1.5 min-w-0">
                  <p class="truncate text-xs font-medium text-slate-300">{collection.name}</p>
                  <div class="mt-0.5 flex items-center justify-between gap-2 text-[9px] text-slate-600">
                    <span>{collection.entryCount} tracks</span>
                    <span class={collection.trackedEntryCount > 0 ? 'text-emerald-500' : ''}>{collectionStatus(collection)}</span>
                  </div>
                </div>
              </button>
            {/each}
          </div>
        {/if}

        {#if section === 'albums' && savedAlbums.length < savedAlbumTotal}
          <div class="flex justify-center py-3">
            <button type="button" onclick={() => onLoadMoreSavedAlbums?.()} disabled={savedAlbumsLoadingMore} class="rounded-md border border-slate-800 px-2.5 py-1 text-[10px] text-slate-500 hover:bg-slate-900 disabled:opacity-50">
              {savedAlbumsLoadingMore ? 'Loading…' : 'Load more albums'}
            </button>
          </div>
        {:else if section === 'playlists' && playlists.length < playlistTotal}
          <div class="flex justify-center py-3">
            <button type="button" onclick={() => onLoadMorePlaylists?.()} disabled={playlistsLoadingMore} class="rounded-md border border-slate-800 px-2.5 py-1 text-[10px] text-slate-500 hover:bg-slate-900 disabled:opacity-50">
              {playlistsLoadingMore ? 'Loading…' : 'Load more playlists'}
            </button>
          </div>
        {/if}

        {#if section === 'playlists' && unavailablePlaylists.length > 0}
          <details class="mt-3 border-t border-slate-900 pt-2.5">
            <summary class="cursor-pointer text-[9px] font-medium uppercase tracking-wider text-slate-700">
              Unavailable · {unavailablePlaylists.length}
            </summary>
            <div class="mt-2 grid gap-1.5 sm:grid-cols-2 xl:grid-cols-3">
              {#each unavailablePlaylists as playlist (playlist.id)}
                <div class="rounded-md bg-slate-950 px-2.5 py-2">
                  <p class="truncate text-[10px] text-slate-600">{playlist.name}</p>
                  <p class="mt-0.5 line-clamp-2 text-[9px] leading-4 text-slate-800">{playlist.accessIssue ?? 'Spotify does not expose this playlist to Refrain.'}</p>
                </div>
              {/each}
            </div>
          </details>
        {/if}
      </div>
    </div>
  {/if}
</div>
