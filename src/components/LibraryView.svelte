<script lang="ts">
  import { convertFileSrc, invoke } from '@tauri-apps/api/core';
  import DataTableHeader from './DataTableHeader.svelte';
  import Icon from './Icon.svelte';
  import OverflowMenu from './OverflowMenu.svelte';
  import type { LibraryTrackRow } from '../lib/library';
  import type { OverflowMenuItem } from '../lib/menu';
  import type { SyncRun } from '../lib/sync';
  import { formatTrackDuration } from '../lib/source';
  import { tableGridWidth, type TableColumn } from '../lib/table-columns';

  export let tracks: LibraryTrackRow[] = [];
  export let total = 0;
  export let loading = false;
  export let loadingMore = false;
  export let error: string | null = null;
  export let onLoadMore: (() => void) | undefined = undefined;
  export let onScan: (() => void) | undefined = undefined;
  export let onShowIssues: (() => void) | undefined = undefined;
  export let scanBusy = false;
  export let scanDisabled = false;
  export let syncRun: SyncRun | null = null;
  export let indexedFiles = 0;
  export let issueCount = 0;

  let search = '';
  let spotifyState = 'all';
  let advancedOpen = false;
  let pathFilter = '';
  let acquisitionFilter = 'all';
  let matchFilter = 'all';
  let explicitFilter = 'all';
  let loadAllRequested = false;
  let lastAutoLoadOffset = -1;
  let sortId = 'title';
  let sortDirection: 'asc' | 'desc' = 'asc';
  $: activeFilterCount = [
    spotifyState !== 'all',
    pathFilter.trim() !== '',
    acquisitionFilter !== 'all',
    matchFilter !== 'all',
    explicitFilter !== 'all',
  ].filter(Boolean).length;
  let columns: TableColumn[] = [
    { id: 'title', label: 'Title', width: 300, minWidth: 220, sortable: true },
    {
      id: 'artist',
      label: 'Artist',
      width: 150,
      minWidth: 100,
      sortable: true,
    },
    { id: 'album', label: 'Album', width: 170, minWidth: 110, sortable: true },
    { id: 'year', label: 'Year', width: 58, minWidth: 48, sortable: true },
    {
      id: 'duration',
      label: 'Duration',
      width: 72,
      minWidth: 60,
      sortable: true,
    },
    { id: 'format', label: 'Format', width: 62, minWidth: 54, sortable: true },
    {
      id: 'spotify',
      label: 'Spotify',
      width: 100,
      minWidth: 84,
      sortable: true,
    },
    {
      id: 'collections',
      label: 'Collections',
      width: 170,
      minWidth: 120,
      sortable: true,
    },
    {
      id: 'actions',
      label: '',
      width: 30,
      minWidth: 30,
      draggable: false,
      align: 'center',
    },
  ];
  $: acquisitionOptions = uniqueSorted(
    tracks.flatMap((track) =>
      track.acquisitionStatus ? [track.acquisitionStatus] : [],
    ),
  );
  $: filteredTracks = tracks.filter(matchesFilters);
  $: sortedTracks = [...filteredTracks].sort((a, b) =>
    compareTracks(a, b, sortId, sortDirection),
  );
  $: gridTemplate = columns.map((column) => `${column.width}px`).join(' ');
  $: tableWidth = tableGridWidth(columns);
  $: matchedCount =
    syncRun?.matched ??
    tracks.filter((track) => track.spotifyMemberships.length > 0).length;
  $: hasMore = tracks.length < total;
  $: filterActive =
    search.trim() !== '' ||
    spotifyState !== 'all' ||
    pathFilter.trim() !== '' ||
    acquisitionFilter !== 'all' ||
    matchFilter !== 'all' ||
    explicitFilter !== 'all';
  $: if (
    (loadAllRequested || filterActive) &&
    hasMore &&
    !loading &&
    !loadingMore &&
    tracks.length !== lastAutoLoadOffset
  ) {
    lastAutoLoadOffset = tracks.length;
    onLoadMore?.();
  }
  $: if (!(loadAllRequested || filterActive) || !hasMore) {
    lastAutoLoadOffset = -1;
  }
  $: if (!hasMore) loadAllRequested = false;

  function uniqueSorted(values: string[]): string[] {
    return [...new Set(values.filter(Boolean))].sort((a, b) =>
      a.localeCompare(b),
    );
  }

  function membershipLabel(kind: string, name: string): string {
    return kind === 'liked_songs' ? 'Liked Songs' : name;
  }

  function artworkUrl(track: LibraryTrackRow): string | null {
    const path = track.preferredFile?.artworkPath;
    return path ? convertFileSrc(path) : null;
  }

  function formatNumber(value: number): string {
    return new Intl.NumberFormat().format(value);
  }

  async function copyText(value: string) {
    await navigator.clipboard.writeText(value);
  }

  function trackMenuItems(track: LibraryTrackRow): OverflowMenuItem[] {
    const items: OverflowMenuItem[] = [];
    const path = track.preferredFile?.path;

    if (path) {
      items.push(
        {
          label: 'Reveal in File Manager',
          icon: 'folder',
          action: () => invoke('reveal_in_file_manager', { path }),
        },
        {
          label: 'Copy File Path',
          icon: 'copy',
          action: () => copyText(path),
        },
      );
    }

    items.push({
      label: 'Copy Track Name',
      icon: 'copy',
      action: () =>
        copyText(
          track.artists.length > 0
            ? `${track.title} — ${track.artists.join(', ')}`
            : track.title,
        ),
    });

    return items;
  }

  function matchesFilters(track: LibraryTrackRow): boolean {
    const query = search.trim().toLocaleLowerCase();
    const filePath = track.preferredFile?.path ?? '';
    if (
      query &&
      ![
        track.title,
        ...track.artists,
        track.album ?? '',
        filePath,
        ...track.spotifyMemberships.map((membership) => membership.name),
      ].some((value) => value.toLocaleLowerCase().includes(query))
    )
      return false;

    const onSpotify = track.spotifyMemberships.length > 0;
    if (spotifyState === 'spotify' && !onSpotify) return false;
    if (spotifyState === 'local' && onSpotify) return false;

    const normalizedPath = pathFilter.trim().toLocaleLowerCase();
    if (
      normalizedPath &&
      !filePath.toLocaleLowerCase().includes(normalizedPath)
    )
      return false;
    if (
      acquisitionFilter !== 'all' &&
      track.acquisitionStatus !== acquisitionFilter
    )
      return false;
    if (matchFilter === 'matched' && track.sourceTrackCount <= 0) return false;
    if (matchFilter === 'unmatched' && track.sourceTrackCount > 0) return false;
    if (explicitFilter === 'explicit' && track.explicit !== true) return false;
    if (explicitFilter === 'clean' && track.explicit === true) return false;

    return true;
  }

  const collator = new Intl.Collator(undefined, {
    numeric: true,
    sensitivity: 'base',
  });

  function sortValue(
    track: LibraryTrackRow,
    currentSortId: string,
  ): string | number {
    switch (currentSortId) {
      case 'artist':
        return track.artists.join(', ');
      case 'album':
        return track.album ?? '';
      case 'year':
        return track.releaseYear ?? -1;
      case 'duration':
        return track.durationMs ?? -1;
      case 'format':
        return track.preferredFile?.format ?? '';
      case 'spotify':
        return track.spotifyMemberships.length > 0 ? 1 : 0;
      case 'collections':
        return track.spotifyMemberships
          .map((membership) =>
            membershipLabel(membership.kind, membership.name),
          )
          .join(', ');
      case 'title':
      default:
        return track.title;
    }
  }

  function compareTracks(
    a: LibraryTrackRow,
    b: LibraryTrackRow,
    currentSortId: string,
    currentSortDirection: 'asc' | 'desc',
  ): number {
    const left = sortValue(a, currentSortId);
    const right = sortValue(b, currentSortId);
    const result =
      typeof left === 'number' && typeof right === 'number'
        ? left - right
        : collator.compare(String(left), String(right));
    return currentSortDirection === 'asc' ? result : -result;
  }

  function clearFilters() {
    spotifyState = 'all';
    pathFilter = '';
    acquisitionFilter = 'all';
    matchFilter = 'all';
    explicitFilter = 'all';
  }

  function setSpotifyState(value: 'all' | 'spotify' | 'local') {
    lastAutoLoadOffset = -1;
    spotifyState = value;
    loadAllRequested = true;
  }
</script>

<div class="screen">
  <header class="page-header">
    <div>
      <div class="page-heading-line">
        <h1 class="page-title">Local Library</h1>
      </div>
      <p class="page-description">
        Tracks that physically exist on your disk, and how they relate to
        Spotify.
      </p>
    </div>
    <div class="page-actions">
      {#if issueCount > 0}
        <button
          type="button"
          class="btn btn-attention"
          onclick={() => onShowIssues?.()}
        >
          <Icon name="warning" size={15} />
          {formatNumber(issueCount)} Local Only
          <Icon name="chevron-right" size={14} />
        </button>
      {/if}
      <button
        type="button"
        class="btn btn-primary"
        onclick={() => onScan?.()}
        disabled={scanBusy || scanDisabled}
      >
        <Icon name="refresh" size={15} />
        {scanBusy ? 'Scanning…' : 'Scan Files'}
      </button>
    </div>
  </header>

  <div class="metrics-strip">
    <div class="metric-inline">
      <div class="metric-value">{formatNumber(total)}</div>
      <div class="metric-label">Total Tracks</div>
    </div>
    <div class="metric-inline">
      <div class="metric-value">{formatNumber(indexedFiles)}</div>
      <div class="metric-label">Indexed Files</div>
    </div>
    <div class="metric-inline">
      <div class="metric-value">{formatNumber(matchedCount)}</div>
      <div class="metric-label">On Spotify</div>
    </div>
    <div class="metric-inline attention">
      <div class="metric-value">{formatNumber(issueCount)}</div>
      <div class="metric-label">Local Only</div>
    </div>
  </div>

  <div class="toolbar local-library-toolbar">
    <label class="search-field">
      <Icon name="search" size={17} />
      <input
        bind:value={search}
        class="search-input"
        aria-label="Search local library"
        placeholder="Search title, artist, album, path, or collection"
      />
    </label>

    <button
      type="button"
      class="btn filter-button"
      class:filters-active={activeFilterCount > 0}
      aria-expanded={advancedOpen}
      onclick={() => (advancedOpen = !advancedOpen)}
    >
      <Icon name="filter" size={15} />
      Filters
      {#if activeFilterCount > 0}
        <span class="filter-count">{activeFilterCount}</span>
      {/if}
      <Icon name="chevron-down" size={13} />
    </button>
  </div>

  {#if advancedOpen}
    <div class="filters-panel">
      <div class="filters-panel-header">
        <div class="filters-panel-title">Filter results</div>
        <button type="button" class="link-button" onclick={clearFilters}>
          Clear
        </button>
      </div>
      <div class="filters-grid">
        <label class="filter-field">
          <span class="filter-label">Spotify Status</span>
          <select
            value={spotifyState}
            aria-label="Spotify state"
            class="filter-select"
            onchange={(event) =>
              setSpotifyState(
                event.currentTarget.value as 'all' | 'spotify' | 'local',
              )}
          >
            <option value="all">Any status</option>
            <option value="spotify">On Spotify</option>
            <option value="local">Local Only</option>
          </select>
        </label>
        <label class="filter-field">
          <span class="filter-label">Path</span>
          <input
            bind:value={pathFilter}
            aria-label="File path filter"
            placeholder="Path contains…"
            class="filter-input"
          />
        </label>
        <label class="filter-field">
          <span class="filter-label">Acquisition</span>
          <select
            bind:value={acquisitionFilter}
            aria-label="Acquisition filter"
            class="filter-select"
          >
            <option value="all">Any state</option>
            {#each acquisitionOptions as status (status)}<option value={status}
                >{status}</option
              >{/each}
          </select>
        </label>
        <label class="filter-field">
          <span class="filter-label">Match</span>
          <select
            bind:value={matchFilter}
            aria-label="Match filter"
            class="filter-select"
          >
            <option value="all">Any state</option>
            <option value="matched">Matched</option>
            <option value="unmatched">Unmatched</option>
          </select>
        </label>
        <label class="filter-field">
          <span class="filter-label">Explicit</span>
          <select
            bind:value={explicitFilter}
            aria-label="Explicit filter"
            class="filter-select"
          >
            <option value="all">Any</option>
            <option value="explicit">Explicit</option>
            <option value="clean">Clean</option>
          </select>
        </label>
      </div>
    </div>
  {/if}

  {#if error}
    <div class="error-state">{error}</div>
  {:else if loading && tracks.length === 0}
    <div class="data-table" aria-label="Loading local library">
      {#each Array.from({ length: 9 }, (_, index) => index) as index (index)}
        <div class="skeleton" style="height: 48px; margin-bottom: 2px;"></div>
      {/each}
    </div>
  {:else if tracks.length === 0}
    <div class="empty-state">
      <div>
        <strong>No local music indexed yet</strong>
        <p>Choose a library folder in Settings, then scan your files.</p>
      </div>
    </div>
  {:else if filteredTracks.length === 0}
    <div class="empty-state">No local tracks match the current filters.</div>
  {:else}
    <div class="data-table">
      <div class="table-scroll">
        <DataTableHeader
          bind:columns
          bind:sortId
          bind:sortDirection
          storageKey="refrain.table.local.columns.v1"
        />
        {#each sortedTracks as track (track.id)}
          {@const art = artworkUrl(track)}
          <article
            class="table-row column-table-grid"
            style={`grid-template-columns:${gridTemplate}; width:${tableWidth}px; min-width:${tableWidth}px;`}
          >
            {#each columns as column (column.id)}
              {#if column.id === 'title'}
                <div class="track-primary">
                  {#if art}
                    <img
                      src={art}
                      alt=""
                      loading="lazy"
                      class="artwork-small"
                    />
                  {:else}
                    <div class="artwork-small artwork-fallback">
                      {(track.artists[0] ?? track.title)
                        .slice(0, 1)
                        .toUpperCase()}
                    </div>
                  {/if}
                  <div class="track-copy">
                    <div class="track-title-line">
                      <p class="track-title">{track.title}</p>
                      {#if track.explicit}<span class="explicit-badge">E</span
                        >{/if}
                    </div>
                    {#if track.preferredFile}
                      <p
                        class="track-secondary mono-path"
                        title={track.preferredFile.path}
                      >
                        {track.preferredFile.path}
                      </p>
                    {:else}
                      <p class="track-secondary">No preferred file</p>
                    {/if}
                  </div>
                </div>
              {:else if column.id === 'artist'}
                <span class="cell-truncate"
                  >{track.artists.join(', ') || 'Unknown artist'}</span
                >
              {:else if column.id === 'album'}
                <span class="cell-truncate"
                  >{track.album ?? 'Unknown album'}</span
                >
              {:else if column.id === 'year'}
                <span>{track.releaseYear ?? '—'}</span>
              {:else if column.id === 'duration'}
                <span>{formatTrackDuration(track.durationMs)}</span>
              {:else if column.id === 'format'}
                <span>{track.preferredFile?.format?.toUpperCase() ?? '—'}</span>
              {:else if column.id === 'spotify'}
                <span class="state-text">
                  <span
                    class:success={track.spotifyMemberships.length > 0}
                    class:warning={track.spotifyMemberships.length === 0}
                    class="state-dot"
                  ></span>
                  {track.spotifyMemberships.length > 0
                    ? 'On Spotify'
                    : 'Local Only'}
                </span>
              {:else if column.id === 'collections'}
                <div class="cell-truncate collection-chip-cell">
                  {#each track.spotifyMemberships.slice(0, 2) as membership (`${membership.kind}:${membership.name}`)}
                    <span class="chip"
                      >{membershipLabel(membership.kind, membership.name)}</span
                    >
                  {/each}
                  {#if track.spotifyMemberships.length > 2}
                    <span class="chip"
                      >+{track.spotifyMemberships.length - 2}</span
                    >
                  {/if}
                  {#if track.spotifyMemberships.length === 0}
                    <span style="color:var(--text-tertiary)">—</span>
                  {/if}
                </div>
              {:else if column.id === 'actions'}
                <OverflowMenu
                  items={trackMenuItems(track)}
                  ariaLabel={`Actions for ${track.title}`}
                />
              {/if}
            {/each}
          </article>
        {/each}
        {#if hasMore}
          <div class="table-load-more-row">
            <button
              type="button"
              class="btn"
              onclick={() => onLoadMore?.()}
              disabled={loadingMore}
            >
              {loadingMore ? 'Loading…' : 'Load More'}
            </button>
          </div>
        {/if}
      </div>
    </div>
  {/if}

  <footer class="table-footer">
    <span>Loaded {formatNumber(tracks.length)} of {formatNumber(total)}</span>
  </footer>
</div>
