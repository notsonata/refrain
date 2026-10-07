<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type {
    LocalPlaylistDetail,
    LocalPlaylistEntry,
    LocalPlaylistSummary,
  } from '../lib/playlists';
  import type { OverflowMenuItem } from '../lib/menu';
  import { formatTrackDuration } from '../lib/source';
  import Icon from './Icon.svelte';
  import OverflowMenu from './OverflowMenu.svelte';

  export let playlists: LocalPlaylistSummary[] = [];
  export let selectedPlaylist: LocalPlaylistDetail | null = null;
  export let loading = false;
  export let busy = false;
  export let error: string | null = null;
  export let onSelect:
    ((playlistId: number) => void | Promise<void>) | undefined = undefined;
  export let onCreate: ((name: string) => void | Promise<void>) | undefined =
    undefined;
  export let onBack: (() => void | Promise<void>) | undefined = undefined;
  export let onRename: ((name: string) => void | Promise<void>) | undefined =
    undefined;
  export let onDelete: (() => void | Promise<void>) | undefined = undefined;
  export let onMoveEntry:
    | ((entryId: number, newPosition: number) => void | Promise<void>)
    | undefined = undefined;
  export let onRemoveEntry:
    ((entryId: number) => void | Promise<void>) | undefined = undefined;

  let creating = false;
  let newPlaylistName = '';
  let playlistSearch = '';
  let trackSearch = '';
  let editingName = false;
  let nameDraft = '';
  let lastSelectedPlaylistId: number | null = null;

  $: selectedIsSpotifyMirror = selectedPlaylist?.sourceCollectionId != null;
  $: filteredPlaylists = filterPlaylists(playlists, playlistSearch);
  $: filteredEntries = filterEntries(
    selectedPlaylist?.entries ?? [],
    trackSearch,
  );
  $: totalTracks = playlists.reduce(
    (total, playlist) => total + playlist.entryCount,
    0,
  );
  $: spotifyMirrorCount = playlists.filter(
    (playlist) => playlist.sourceCollectionId != null,
  ).length;

  $: if (selectedPlaylist?.id !== lastSelectedPlaylistId) {
    lastSelectedPlaylistId = selectedPlaylist?.id ?? null;
    nameDraft = selectedPlaylist?.name ?? '';
    trackSearch = '';
    editingName = false;
  }

  function filterPlaylists(
    items: LocalPlaylistSummary[],
    query: string,
  ): LocalPlaylistSummary[] {
    const normalized = query.trim().toLocaleLowerCase();
    if (!normalized) return items;
    return items.filter((playlist) =>
      playlist.name.toLocaleLowerCase().includes(normalized),
    );
  }

  function filterEntries(
    entries: LocalPlaylistEntry[],
    query: string,
  ): LocalPlaylistEntry[] {
    const normalized = query.trim().toLocaleLowerCase();
    if (!normalized) return entries;
    return entries.filter((entry) =>
      [entry.title, entry.album ?? '', ...entry.artists]
        .join(' ')
        .toLocaleLowerCase()
        .includes(normalized),
    );
  }

  function formatSyncTime(timestamp: number | null): string {
    if (!timestamp) return 'Waiting for first sync';
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(new Date(timestamp));
  }

  function playlistSyncLabel(playlist: LocalPlaylistSummary): string {
    if (playlist.syncError) return 'Needs attention';
    if (!playlist.m3uPath) return 'M3U8 pending';
    return playlist.m3uManaged ? 'M3U8 synced' : 'Linked M3U8';
  }

  function artworkUrl(entry: LocalPlaylistEntry): string | null {
    return entry.artworkPath ? convertFileSrc(entry.artworkPath) : null;
  }

  async function createPlaylist() {
    const name = newPlaylistName.trim();
    if (!name || busy) return;
    await onCreate?.(name);
    newPlaylistName = '';
    creating = false;
  }

  async function commitRename() {
    const name = nameDraft.trim();
    if (!name || !selectedPlaylist || busy) return;
    if (name !== selectedPlaylist.name) await onRename?.(name);
    editingName = false;
  }

  function playlistMenuItems(): OverflowMenuItem[] {
    if (!selectedPlaylist || selectedPlaylist.sourceCollectionId != null)
      return [];
    return [
      {
        label: 'Rename Playlist',
        icon: 'refresh',
        action: () => {
          nameDraft = selectedPlaylist?.name ?? '';
          editingName = true;
        },
      },
      {
        label: 'Delete Playlist',
        icon: 'close',
        action: () => onDelete?.(),
        separatorBefore: true,
      },
    ];
  }

  function entryMenuItems(entry: LocalPlaylistEntry): OverflowMenuItem[] {
    if (!selectedPlaylist || selectedPlaylist.sourceCollectionId != null)
      return [];
    const index = selectedPlaylist.entries.findIndex(
      (item) => item.id === entry.id,
    );
    return [
      {
        label: 'Move Up',
        icon: 'sort-asc',
        action: () => onMoveEntry?.(entry.id, Math.max(0, index - 1)),
        disabled: index <= 0,
      },
      {
        label: 'Move Down',
        icon: 'sort-desc',
        action: () => onMoveEntry?.(entry.id, index + 1),
        disabled: index < 0 || index >= selectedPlaylist.entries.length - 1,
      },
      {
        label: 'Remove from Playlist',
        icon: 'close',
        action: () => onRemoveEntry?.(entry.id),
        separatorBefore: true,
      },
    ];
  }
</script>

<div class="screen local-playlists-screen">
  {#if selectedPlaylist}
    <div
      class="toolbar detail-toolbar local-playlist-detail-toolbar"
      style="padding-top:2px; padding-bottom:12px; border-bottom:1px solid var(--divider);"
    >
      <div class="detail-breadcrumb">
        <button
          type="button"
          class="icon-button"
          aria-label="Back to local playlists"
          title="Back to local playlists"
          onclick={() => onBack?.()}
          disabled={busy}
        >
          <Icon name="back" size={17} />
        </button>
        <strong>Local</strong>
        <Icon name="chevron-right" size={13} />
        <span>Playlists</span>
        <Icon name="chevron-right" size={13} />
        <span class="detail-breadcrumb-current" title={selectedPlaylist.name}
          >{selectedPlaylist.name}</span
        >
      </div>
    </div>

    {#if error}
      <div class="error-state local-playlist-error">{error}</div>
    {/if}

    <section class="local-playlist-workspace" aria-label="Playlist details">
      <header class="album-detail-header local-playlist-detail-header">
        <div class="album-detail-title-row">
          {#if editingName}
            <form
              class="local-playlist-rename"
              onsubmit={(event) => {
                event.preventDefault();
                void commitRename();
              }}
            >
              <input
                bind:value={nameDraft}
                class="local-playlist-name-input"
                aria-label="Playlist name"
                onblur={() => void commitRename()}
              />
            </form>
          {:else}
            <h1 class="album-detail-title">{selectedPlaylist.name}</h1>
          {/if}
        </div>

        <div class="album-detail-meta-row">
          <div class="album-status-strip" aria-label="Local playlist status">
            <span class="album-status-item success">
              <Icon name="check" size={14} />
              <strong>{selectedPlaylist.entries.length.toLocaleString()}</strong
              >
              {selectedPlaylist.entries.length === 1 ? 'Track' : 'Tracks'}
            </span>
            <span class="album-status-item">
              {#if selectedIsSpotifyMirror}
                <Icon name="spotify" size={13} />
                <strong>Spotify mirror</strong>
              {:else}
                <Icon name="local" size={14} />
                <strong>Local playlist</strong>
              {/if}
            </span>
            <span
              class:attention={Boolean(selectedPlaylist.syncError)}
              class="album-status-item"
            >
              <Icon
                name={selectedPlaylist.syncError ? 'warning' : 'sync'}
                size={14}
              />
              <strong>
                {selectedPlaylist.syncError
                  ? 'M3U8 needs attention'
                  : selectedPlaylist.m3uPath
                    ? 'M3U8 synced'
                    : 'M3U8 pending'}
              </strong>
            </span>
          </div>

          {#if !selectedIsSpotifyMirror}
            <div class="album-detail-actions">
              <OverflowMenu
                items={playlistMenuItems()}
                ariaLabel="Playlist actions"
              />
            </div>
          {/if}
        </div>
      </header>

      <div class="local-playlist-managed-file">
        <div class="local-playlist-managed-file-copy">
          <span class="local-playlist-managed-label">M3U8</span>
          <span
            class="local-playlist-managed-path"
            title={selectedPlaylist.m3uPath ?? ''}
          >
            {selectedPlaylist.m3uPath ??
              'Waiting for the library folder and a complete local copy.'}
          </span>
        </div>
        <span class="local-playlist-managed-time">
          Synced {formatSyncTime(selectedPlaylist.lastSyncedAt)}
        </span>
      </div>

      {#if selectedPlaylist.syncError}
        <div class="local-playlist-sync-error">
          <Icon name="warning" size={14} />
          <span>{selectedPlaylist.syncError}</span>
        </div>
      {/if}

      <div class="toolbar track-list-toolbar local-playlist-track-toolbar">
        <label class="search-field">
          <Icon name="search" size={17} />
          <input
            bind:value={trackSearch}
            class="search-input"
            aria-label="Search playlist tracks"
            placeholder="Search title, artist, or album"
          />
        </label>
        <span class="local-playlist-toolbar-count">
          {filteredEntries.length.toLocaleString()} of
          {selectedPlaylist.entries.length.toLocaleString()} tracks
        </span>
      </div>

      {#if loading}
        <div class="local-playlist-detail-loading">
          <div class="skeleton" style="height:54px"></div>
          <div class="skeleton" style="height:54px"></div>
          <div class="skeleton" style="height:54px"></div>
        </div>
      {:else if selectedPlaylist.entries.length === 0}
        <div class="empty-state local-playlist-empty">
          <div>
            <strong>This playlist is empty</strong>
            <p>
              {selectedIsSpotifyMirror
                ? 'Tracked Spotify entries will appear here after they resolve into the local library.'
                : 'Go to Local > Songs, select tracks, then add them to this playlist.'}
            </p>
          </div>
        </div>
      {:else if filteredEntries.length === 0}
        <div class="empty-state local-playlist-empty">
          <div>
            <strong>No matching tracks</strong>
            <p>Try a different title, artist, or album.</p>
          </div>
        </div>
      {:else}
        <div
          class="data-table spotify-track-table local-playlist-track-table"
          role="table"
          aria-label="Playlist tracks"
        >
          <div class="table-head local-playlist-track-head" role="row">
            <span>#</span>
            <span>Track</span>
            <span>Duration</span>
            <span>Local</span>
            <span aria-hidden="true"></span>
          </div>
          <div class="local-playlist-track-list" role="rowgroup">
            {#each filteredEntries as entry (entry.id)}
              <div class="table-row local-playlist-track-row" role="row">
                <span class="local-playlist-position" role="cell">
                  {entry.position + 1}
                </span>
                <div class="local-playlist-track-main" role="cell">
                  <span
                    class="artwork-small artwork-fallback local-playlist-artwork"
                  >
                    {#if artworkUrl(entry)}
                      <img src={artworkUrl(entry) ?? ''} alt="" />
                    {:else}
                      <Icon name="album" size={17} />
                    {/if}
                  </span>
                  <span class="local-playlist-track-copy">
                    <strong class="track-title">{entry.title}</strong>
                    <span class="track-secondary">
                      {entry.artists.join(', ') || 'Unknown artist'}
                      {entry.album ? ` · ${entry.album}` : ''}
                    </span>
                  </span>
                </div>
                <span class="local-playlist-duration" role="cell">
                  {formatTrackDuration(entry.durationMs)}
                </span>
                <span
                  class:missing={!entry.filePath}
                  class="local-playlist-file-state"
                  role="cell"
                >
                  <span class="local-playlist-file-dot"></span>
                  {entry.filePath ? 'Ready' : 'Missing'}
                </span>
                <span class="local-playlist-row-actions" role="cell">
                  {#if !selectedIsSpotifyMirror}
                    <OverflowMenu
                      items={entryMenuItems(entry)}
                      ariaLabel={`Actions for ${entry.title}`}
                    />
                  {/if}
                </span>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </section>
  {:else}
    <header class="page-header local-playlists-browser-header">
      <div>
        <div class="page-heading-line">
          <h1 class="page-title">Local Playlists</h1>
        </div>
        <p class="page-description">
          Local playlists and tracked Spotify mirrors, kept as playable M3U8
          files.
        </p>
      </div>
      <div class="page-actions">
        <button
          type="button"
          class="btn btn-primary"
          onclick={() => (creating = true)}
          disabled={busy || creating}
        >
          <Icon name="plus" size={15} />
          New Playlist
        </button>
      </div>
    </header>

    <div
      class="metrics-strip local-playlist-metrics"
      aria-label="Playlist summary"
    >
      <div class="metric-inline">
        <div class="metric-value">{playlists.length.toLocaleString()}</div>
        <div class="metric-label">
          total {playlists.length === 1 ? 'playlist' : 'playlists'}
        </div>
      </div>
      <div class="metric-inline">
        <div class="metric-value">{totalTracks.toLocaleString()}</div>
        <div class="metric-label">Tracks</div>
      </div>
      <div class="metric-inline">
        <div class="metric-value">{spotifyMirrorCount.toLocaleString()}</div>
        <div class="metric-label">Spotify Mirrors</div>
      </div>
    </div>

    {#if error}
      <div class="error-state local-playlist-error">{error}</div>
    {/if}

    {#if creating}
      <form
        class="local-playlist-create"
        onsubmit={(event) => {
          event.preventDefault();
          void createPlaylist();
        }}
      >
        <input
          bind:value={newPlaylistName}
          class="filter-input"
          aria-label="New playlist name"
          placeholder="Playlist name"
        />
        <button
          type="submit"
          class="btn btn-primary"
          disabled={!newPlaylistName.trim() || busy}>Create</button
        >
        <button
          type="button"
          class="btn"
          onclick={() => {
            creating = false;
            newPlaylistName = '';
          }}
          disabled={busy}>Cancel</button
        >
      </form>
    {/if}

    <div class="toolbar local-playlists-toolbar">
      <label class="search-field">
        <Icon name="search" size={17} />
        <input
          bind:value={playlistSearch}
          class="search-input"
          aria-label="Search local playlists"
          placeholder="Search playlists"
        />
      </label>
      <span class="local-playlist-toolbar-count">
        {filteredPlaylists.length.toLocaleString()} shown
      </span>
    </div>

    {#if loading}
      <div class="local-playlist-browser-loading">
        <div class="skeleton" style="height:62px"></div>
        <div class="skeleton" style="height:62px"></div>
        <div class="skeleton" style="height:62px"></div>
      </div>
    {:else if playlists.length === 0}
      <div class="empty-state local-playlist-empty">
        <div>
          <strong>No playlists yet</strong>
          <p>
            Create a local playlist or track a Spotify playlist to add one here.
          </p>
        </div>
      </div>
    {:else if filteredPlaylists.length === 0}
      <div class="empty-state local-playlist-empty">
        <div>
          <strong>No matching playlists</strong>
          <p>Try a different playlist name.</p>
        </div>
      </div>
    {:else}
      <div class="collection-layout local-playlist-collection-layout">
        <div class="collection-scroll" aria-label="Local playlists">
          <div class="collection-grid">
            {#each filteredPlaylists as playlist (playlist.id)}
              <button
                type="button"
                class="collection-card local-playlist-card"
                onclick={() => onSelect?.(playlist.id)}
              >
                <div class="collection-artwork">
                  {#if playlist.imageUrl}
                    <img src={playlist.imageUrl} alt="" loading="lazy" />
                  {:else}
                    <div
                      class="artwork-fallback"
                      style="width:100%; height:100%; font-size:26px;"
                    >
                      {playlist.name.slice(0, 1).toUpperCase()}
                    </div>
                  {/if}
                  <span
                    class="local-playlist-source-badge"
                    title={playlist.sourceCollectionId != null
                      ? 'Spotify mirror'
                      : 'Local playlist'}
                  >
                    <Icon
                      name={playlist.sourceCollectionId != null
                        ? 'spotify'
                        : 'local'}
                      size={12}
                    />
                  </span>
                </div>
                <p class="collection-name">{playlist.name}</p>
                <div class="collection-meta">
                  <span>{playlist.entryCount.toLocaleString()} tracks</span>
                  <span
                    class="chip"
                    class:attention={Boolean(playlist.syncError)}
                    class:success={!playlist.syncError &&
                      Boolean(playlist.m3uPath)}
                  >
                    {playlistSyncLabel(playlist)}
                  </span>
                </div>
                <p class="local-playlist-card-source">
                  {playlist.sourceCollectionId != null
                    ? 'Spotify mirror'
                    : 'Local playlist'}
                </p>
              </button>
            {/each}
          </div>
        </div>
      </div>
    {/if}
  {/if}
</div>
