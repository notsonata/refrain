<script lang="ts">
  import { convertFileSrc, invoke } from '@tauri-apps/api/core';
  import { onDestroy } from 'svelte';
  import DataTableHeader from './DataTableHeader.svelte';
  import CollectionChips from './CollectionChips.svelte';
  import Icon from './Icon.svelte';
  import OverflowMenu from './OverflowMenu.svelte';
  import TrackStatusView from './TrackStatus.svelte';
  import type { LibraryTrackRow } from '../lib/library';
  import type { OverflowMenuItem } from '../lib/menu';
  import type { LocalPlaylistSummary } from '../lib/playlists';
  import type { SyncRun } from '../lib/sync';
  import { formatTrackDuration } from '../lib/source';
  import {
    createLocalTrackColumns,
    sharedTrackColumnIds,
    tableGridTemplate,
    type TableColumn,
  } from '../lib/table-columns';
  import {
    localLibraryTrackStatus,
    localLibraryTrackStatuses,
    trackStatusLabel,
    trackStatusSortValue,
    type TrackStatus,
  } from '../lib/track-status';

  export let tracks: LibraryTrackRow[] = [];
  export let total = 0;
  export let loading = false;
  export let loadingMore = false;
  export let error: string | null = null;
  export let onLoadMore: (() => void) | undefined = undefined;
  export let onScan: (() => void) | undefined = undefined;
  export let scanBusy = false;
  export let scanDisabled = false;
  export let syncRun: SyncRun | null = null;
  export let indexedFiles = 0;
  export let localOnlyCount = 0;
  export let localPlaylists: LocalPlaylistSummary[] = [];
  export let onAddToPlaylist:
    | ((playlistId: number, libraryTrackIds: number[]) => void | Promise<void>)
    | undefined = undefined;

  let search = '';
  let statusFilter: 'all' | TrackStatus = 'all';
  let advancedOpen = false;
  let pathFilter = '';
  let acquisitionFilter = 'all';
  let matchFilter = 'all';
  let explicitFilter = 'all';
  let loadAllRequested = false;
  let lastAutoLoadOffset = -1;
  let sortId = 'title';
  let sortDirection: 'asc' | 'desc' = 'asc';
  let tableViewportWidth = 0;
  let selectedTrackIds = new Set<number>();
  const rowHeight = 50;
  const overscan = 8;
  const numberFormatter = new Intl.NumberFormat();
  let scrollTop = 0;
  let pendingScrollTop = 0;
  let scrollFrame = 0;
  let viewportHeight = 520;
  $: compactRows = tableViewportWidth > 0 && tableViewportWidth <= 820;
  $: activeFilterCount = [
    statusFilter !== 'all',
    pathFilter.trim() !== '',
    acquisitionFilter !== 'all',
    matchFilter !== 'all',
    explicitFilter !== 'all',
  ].filter(Boolean).length;
  let columns: TableColumn[] = createLocalTrackColumns();
  $: acquisitionOptions = uniqueSorted(
    tracks.flatMap((track) =>
      track.acquisitionStatus ? [track.acquisitionStatus] : [],
    ),
  );
  $: filteredTracks = tracks.filter((track) =>
    matchesFilters(
      track,
      search,
      statusFilter,
      pathFilter,
      acquisitionFilter,
      matchFilter,
      explicitFilter,
    ),
  );
  $: sortedTracks = [...filteredTracks].sort((a, b) =>
    compareTracks(a, b, sortId, sortDirection),
  );
  $: if (sortedTracks.length === 0) scrollTop = 0;
  $: visibleCount = Math.ceil(viewportHeight / rowHeight) + overscan * 2;
  $: maxStartIndex = Math.max(0, sortedTracks.length - visibleCount);
  $: startIndex = Math.min(
    Math.max(0, Math.floor(scrollTop / rowHeight) - overscan),
    maxStartIndex,
  );
  $: endIndex = Math.min(sortedTracks.length, startIndex + visibleCount);
  $: visibleTracks = sortedTracks.slice(startIndex, endIndex);
  $: selectedTracks = tracks.filter((track) => selectedTrackIds.has(track.id));
  $: selectedTracksWithPaths = selectedTracks.filter(
    (track) => !!track.preferredFile?.path,
  );
  $: editableLocalPlaylists = localPlaylists.filter(
    (playlist) => playlist.sourceCollectionId === null,
  );
  $: allFilteredSelected =
    filteredTracks.length > 0 &&
    filteredTracks.every((track) => selectedTrackIds.has(track.id));
  $: selectionMenuItems = buildSelectionMenuItems(
    selectedTracks.length,
    selectedTracksWithPaths.length,
    filteredTracks.length,
    allFilteredSelected,
    editableLocalPlaylists,
  );
  $: gridTemplate = tableGridTemplate(columns);
  $: matchedCount =
    syncRun?.matched ??
    tracks.filter((track) => track.spotifyMemberships.length > 0).length;
  $: hasMore = tracks.length < total;
  $: filterActive =
    search.trim() !== '' ||
    statusFilter !== 'all' ||
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

  function updateScrollTop(event: Event) {
    pendingScrollTop = (event.currentTarget as HTMLDivElement).scrollTop;
    if (scrollFrame) return;
    scrollFrame = requestAnimationFrame(() => {
      scrollTop = pendingScrollTop;
      scrollFrame = 0;
    });
  }

  onDestroy(() => {
    if (scrollFrame) cancelAnimationFrame(scrollFrame);
  });

  function uniqueSorted(values: string[]): string[] {
    return [...new Set(values.filter(Boolean))].sort((a, b) =>
      a.localeCompare(b),
    );
  }

  function playlistLabels(track: LibraryTrackRow): string[] {
    return track.spotifyMemberships
      .filter((membership) => membership.kind === 'playlist')
      .map((membership) => membership.name);
  }

  function artworkUrl(track: LibraryTrackRow): string | null {
    const path = track.preferredFile?.artworkPath;
    return path ? convertFileSrc(path) : null;
  }

  function formatNumber(value: number): string {
    return numberFormatter.format(value);
  }

  async function copyText(value: string) {
    await navigator.clipboard.writeText(value);
  }

  function toggleTrackSelection(track: LibraryTrackRow, selected: boolean) {
    const next = new Set(selectedTrackIds);
    if (selected) next.add(track.id);
    else next.delete(track.id);
    selectedTrackIds = next;
  }

  function toggleSelectAllShown() {
    if (filteredTracks.length === 0) return;
    const next = new Set(selectedTrackIds);
    if (allFilteredSelected) {
      for (const track of filteredTracks) next.delete(track.id);
    } else {
      for (const track of filteredTracks) next.add(track.id);
    }
    selectedTrackIds = next;
  }

  function clearSelection() {
    if (selectedTrackIds.size === 0) return;
    selectedTrackIds = new Set();
  }

  async function copySelectedFilePaths() {
    const paths = selectedTracksWithPaths.flatMap((track) =>
      track.preferredFile?.path ? [track.preferredFile.path] : [],
    );
    if (paths.length === 0) return;
    await copyText(paths.join('\n'));
  }

  async function copySelectedTrackNames() {
    if (selectedTracks.length === 0) return;
    await copyText(
      selectedTracks
        .map((track) =>
          track.artists.length > 0
            ? `${track.title} — ${track.artists.join(', ')}`
            : track.title,
        )
        .join('\n'),
    );
  }

  async function addSelectedTracksToPlaylist(playlistId: number) {
    if (selectedTracks.length === 0) return;
    await onAddToPlaylist?.(
      playlistId,
      selectedTracks.map((track) => track.id),
    );
    clearSelection();
  }

  function buildSelectionMenuItems(
    selectedCount: number,
    selectedPathCount: number,
    shownCount: number,
    allShownSelected: boolean,
    playlists: LocalPlaylistSummary[],
  ): OverflowMenuItem[] {
    const items: OverflowMenuItem[] = [
      {
        label: allShownSelected ? 'Clear All Shown' : 'Select All Shown',
        icon: allShownSelected ? 'close' : 'check',
        action: toggleSelectAllShown,
        disabled: shownCount === 0,
      },
      {
        label: 'Clear Selection',
        icon: 'close',
        action: clearSelection,
        disabled: selectedCount === 0,
      },
      {
        label: 'Copy Selected File Paths',
        icon: 'copy',
        action: copySelectedFilePaths,
        disabled: selectedPathCount === 0,
        separatorBefore: true,
      },
      {
        label: 'Copy Selected Track Names',
        icon: 'copy',
        action: copySelectedTrackNames,
        disabled: selectedCount === 0,
      },
    ];
    for (const [index, playlist] of playlists.entries()) {
      items.push({
        label: `Add to ${playlist.name}`,
        icon: 'playlist',
        action: () => addSelectedTracksToPlaylist(playlist.id),
        disabled: selectedCount === 0,
        separatorBefore: index === 0,
      });
    }
    return items;
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

    for (const [index, playlist] of editableLocalPlaylists.entries()) {
      items.push({
        label: `Add to ${playlist.name}`,
        icon: 'playlist',
        action: () => onAddToPlaylist?.(playlist.id, [track.id]),
        separatorBefore: index === 0,
      });
    }

    return items;
  }

  function matchesFilters(
    track: LibraryTrackRow,
    currentSearch: string,
    currentStatusFilter: 'all' | TrackStatus,
    currentPathFilter: string,
    currentAcquisitionFilter: string,
    currentMatchFilter: string,
    currentExplicitFilter: string,
  ): boolean {
    const query = currentSearch.trim().toLocaleLowerCase();
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

    if (
      currentStatusFilter !== 'all' &&
      localLibraryTrackStatus(track.spotifyMemberships.length > 0) !==
        currentStatusFilter
    )
      return false;

    const normalizedPath = currentPathFilter.trim().toLocaleLowerCase();
    if (
      normalizedPath &&
      !filePath.toLocaleLowerCase().includes(normalizedPath)
    )
      return false;
    if (
      currentAcquisitionFilter !== 'all' &&
      track.acquisitionStatus !== currentAcquisitionFilter
    )
      return false;
    if (currentMatchFilter === 'matched' && track.sourceTrackCount <= 0)
      return false;
    if (currentMatchFilter === 'unmatched' && track.sourceTrackCount > 0)
      return false;
    if (currentExplicitFilter === 'explicit' && track.explicit !== true)
      return false;
    if (currentExplicitFilter === 'clean' && track.explicit === true)
      return false;

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
      case 'status':
        return trackStatusSortValue(
          localLibraryTrackStatus(track.spotifyMemberships.length > 0),
        );
      case 'playlists':
        return playlistLabels(track).join(', ');
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
    statusFilter = 'all';
    pathFilter = '';
    acquisitionFilter = 'all';
    matchFilter = 'all';
    explicitFilter = 'all';
  }

  function setStatusFilter(value: 'all' | TrackStatus) {
    lastAutoLoadOffset = -1;
    statusFilter = value;
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
      <button
        type="button"
        class="btn btn-primary"
        onclick={() => onScan?.()}
        disabled={scanBusy || scanDisabled}
      >
        <span class="scan-button-icon" class:is-spinning={scanBusy}>
          <Icon name="refresh" size={15} />
        </span>
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
      <div class="metric-label">Local + Spotify</div>
    </div>
    <div class="metric-inline attention">
      <div class="metric-value">{formatNumber(localOnlyCount)}</div>
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
      <span class="disclosure-chevron" class:expanded={advancedOpen}>
        <Icon name="chevron-right" size={13} />
      </span>
    </button>
    <OverflowMenu
      items={selectionMenuItems}
      ariaLabel="Local library selection actions"
    />
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
          <span class="filter-label">Status</span>
          <select
            value={statusFilter}
            aria-label="Track status"
            class="filter-select"
            onchange={(event) =>
              setStatusFilter(event.currentTarget.value as 'all' | TrackStatus)}
          >
            <option value="all">Any status</option>
            {#each localLibraryTrackStatuses as status (status)}
              <option value={status}>{trackStatusLabel(status)}</option>
            {/each}
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
    <div
      class="data-table local-library-table"
      class:compact={compactRows}
      bind:clientWidth={tableViewportWidth}
    >
      <DataTableHeader
        bind:columns
        bind:sortId
        bind:sortDirection
        compact={compactRows}
        storageKey="refrain.table.local.columns.v5"
        sharedStorageKey="refrain.table.track-columns.v2"
        sharedColumnIds={sharedTrackColumnIds}
      />
      <div
        class="table-scroll local-library-scroll"
        style="position:relative;"
        bind:clientHeight={viewportHeight}
        onscroll={updateScrollTop}
      >
        {#if compactRows}
          <div class="track-card-list">
            {#each sortedTracks as track (track.id)}
              {@const art = artworkUrl(track)}
              <article
                class="track-card local-track-card"
                class:selected={selectedTrackIds.has(track.id)}
              >
                <div class="track-card-main">
                  <label
                    class="track-selection-cell local-track-selection-cell"
                  >
                    <input
                      type="checkbox"
                      checked={selectedTrackIds.has(track.id)}
                      aria-label={`Select ${track.title}`}
                      onchange={(event) =>
                        toggleTrackSelection(
                          track,
                          event.currentTarget.checked,
                        )}
                    />
                  </label>
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
                  <div class="track-card-copy">
                    <div class="track-title-line">
                      <p class="track-title">{track.title}</p>
                      {#if track.explicit}<span class="explicit-badge">E</span
                        >{/if}
                    </div>
                    <p class="track-card-context">
                      {track.artists.join(', ') || 'Unknown artist'} · {track.album ??
                        'Unknown album'}
                    </p>
                    <p
                      class="track-secondary mono-path"
                      title={track.preferredFile?.path ?? 'No preferred file'}
                    >
                      {track.preferredFile?.path ?? 'No preferred file'}
                    </p>
                  </div>
                </div>
                <div class="track-card-actions">
                  <OverflowMenu
                    items={trackMenuItems(track)}
                    ariaLabel={`Actions for ${track.title}`}
                  />
                </div>
                <div class="track-card-meta" aria-label="Track metadata">
                  <span>{track.releaseYear ?? 'Unknown year'}</span>
                  <span>{formatTrackDuration(track.durationMs)}</span>
                  <span
                    >{track.preferredFile?.format?.toUpperCase() ??
                      'Unknown format'}</span
                  >
                </div>
                <div class="track-card-footer">
                  <TrackStatusView
                    status={localLibraryTrackStatus(
                      track.spotifyMemberships.length > 0,
                    )}
                  />
                  <div class="track-card-collections">
                    <CollectionChips labels={playlistLabels(track)} />
                  </div>
                </div>
              </article>
            {/each}
          </div>
        {:else}
          <div
            class="relative"
            style={`height:${sortedTracks.length * rowHeight}px; width:100%; min-width:0;`}
          >
            {#each visibleTracks as track, visibleIndex (track.id)}
              {@const art = artworkUrl(track)}
              <article
                class="table-row column-table-grid absolute left-0 right-0"
                class:selected={selectedTrackIds.has(track.id)}
                style={`height:${rowHeight}px; top:${(startIndex + visibleIndex) * rowHeight}px; grid-template-columns:${gridTemplate};`}
              >
                {#each columns as column (column.id)}
                  {#if column.id === 'order'}
                    <div class="track-order-cell">
                      <label
                        class="track-selection-cell local-track-selection-cell"
                      >
                        <input
                          type="checkbox"
                          checked={selectedTrackIds.has(track.id)}
                          aria-label={`Select ${track.title}`}
                          onchange={(event) =>
                            toggleTrackSelection(
                              track,
                              event.currentTarget.checked,
                            )}
                        />
                      </label>
                    </div>
                  {:else if column.id === 'title'}
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
                          {#if track.explicit}<span class="explicit-badge"
                              >E</span
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
                    <span
                      >{track.preferredFile?.format?.toUpperCase() ?? '—'}</span
                    >
                  {:else if column.id === 'status'}
                    <TrackStatusView
                      status={localLibraryTrackStatus(
                        track.spotifyMemberships.length > 0,
                      )}
                    />
                  {:else if column.id === 'playlists'}
                    {@const playlists = playlistLabels(track)}
                    <div class="collection-chip-cell">
                      <CollectionChips labels={playlists} />
                      {#if playlists.length === 0}
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
          </div>
        {/if}
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
