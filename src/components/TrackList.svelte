<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import DataTableHeader from './DataTableHeader.svelte';
  import Icon from './Icon.svelte';
  import OverflowMenu from './OverflowMenu.svelte';
  import type { OverflowMenuItem } from '../lib/menu';
  import {
    formatTrackDuration,
    type SourceCollectionEntryView,
  } from '../lib/source';
  import { tableGridWidth, type TableColumn } from '../lib/table-columns';

  export let entries: SourceCollectionEntryView[] = [];
  export let total = 0;
  export let loading = false;
  export let loadingMore = false;
  export let emptyMessage = 'No tracks in this collection.';
  export let trackingControls = false;
  export let trackingBusyId: number | null = null;
  export let trackingBulkBusy = false;
  export let showFilters = true;
  export let onLoadMore: (() => void) | undefined = undefined;
  export let onSetTracking:
    | ((
        entry: SourceCollectionEntryView,
        included: boolean | null,
      ) => void | Promise<void>)
    | undefined = undefined;
  export let onSetTrackingMany:
    | ((
        entries: SourceCollectionEntryView[],
        included: boolean | null,
      ) => void | Promise<void>)
    | undefined = undefined;
  export let onSelectionChange:
    | ((state: {
        selectedCount: number;
        selectableFilteredCount: number;
        allFilteredSelected: boolean;
      }) => void)
    | undefined = undefined;

  let search = '';
  let trackingFilter = 'all';
  let localFilter = 'all';
  let advancedOpen = false;
  let acquisitionFilter = 'all';
  let matchFilter = 'all';
  let explicitFilter = 'all';
  let loadAllRequested = false;
  let lastAutoLoadOffset = -1;
  let sortId = 'position';
  let sortDirection: 'asc' | 'desc' = 'asc';
  let selectedEntryKeys = new Set<string>();
  $: activeFilterCount = [
    trackingFilter !== 'all',
    localFilter !== 'all',
    acquisitionFilter !== 'all',
    matchFilter !== 'all',
    explicitFilter !== 'all',
  ].filter(Boolean).length;
  let columns: TableColumn[] = [
    {
      id: 'position',
      label: '#',
      width: trackingControls ? 58 : 38,
      minWidth: trackingControls ? 54 : 34,
      sortable: true,
      align: 'center',
    },
    { id: 'title', label: 'Title', width: 280, minWidth: 220, sortable: true },
    {
      id: 'tracking',
      label: 'Tracking',
      width: 112,
      minWidth: 96,
      sortable: true,
    },
    {
      id: 'local',
      label: 'Local Status',
      width: 108,
      minWidth: 92,
      sortable: true,
    },
    {
      id: 'artist',
      label: 'Artist',
      width: 145,
      minWidth: 100,
      sortable: true,
    },
    { id: 'album', label: 'Album', width: 165, minWidth: 110, sortable: true },
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
      id: 'actions',
      label: '',
      width: 30,
      minWidth: 30,
      draggable: false,
      align: 'center',
    },
  ];

  const rowHeight = 50;
  const overscan = 8;
  const skeletonRows = Array.from({ length: 9 }, (_, index) => index);
  let scrollTop = 0;
  let viewportHeight = 520;

  $: tracks = entries.flatMap((entry) => (entry.track ? [entry.track] : []));
  $: acquisitionOptions = uniqueSorted(
    tracks.flatMap((track) =>
      track.acquisitionStatus ? [track.acquisitionStatus] : [],
    ),
  );
  $: filteredEntries = entries.filter(matchesFilters);
  $: sortedEntries = [...filteredEntries].sort((a, b) =>
    compareEntries(a, b, sortId, sortDirection),
  );
  $: selectedEntries = entries.filter(
    (entry) => entry.track && selectedEntryKeys.has(entrySelectionKey(entry)),
  );
  $: selectedCount = selectedEntries.length;
  $: selectableFilteredEntries = filteredEntries.filter((entry) => entry.track);
  $: selectableFilteredCount = selectableFilteredEntries.length;
  $: allFilteredSelected =
    selectableFilteredEntries.length > 0 &&
    selectableFilteredEntries.every((entry) =>
      selectedEntryKeys.has(entrySelectionKey(entry)),
    );
  $: onSelectionChange?.({
    selectedCount,
    selectableFilteredCount,
    allFilteredSelected,
  });
  $: if (sortedEntries.length === 0) scrollTop = 0;
  $: gridTemplate = columns.map((column) => `${column.width}px`).join(' ');
  $: tableWidth = tableGridWidth(columns);
  $: visibleCount = Math.ceil(viewportHeight / rowHeight) + overscan * 2;
  $: maxStartIndex = Math.max(0, sortedEntries.length - visibleCount);
  $: startIndex = Math.min(
    Math.max(0, Math.floor(scrollTop / rowHeight) - overscan),
    maxStartIndex,
  );
  $: endIndex = Math.min(sortedEntries.length, startIndex + visibleCount);
  $: visibleEntries = sortedEntries.slice(startIndex, endIndex);
  $: hasMore = entries.length < total;
  $: filterActive =
    search.trim() !== '' ||
    trackingFilter !== 'all' ||
    localFilter !== 'all' ||
    acquisitionFilter !== 'all' ||
    matchFilter !== 'all' ||
    explicitFilter !== 'all';
  $: if (
    (loadAllRequested || filterActive) &&
    hasMore &&
    !loading &&
    !loadingMore &&
    entries.length !== lastAutoLoadOffset
  ) {
    lastAutoLoadOffset = entries.length;
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

  function needsAttention(entry: SourceCollectionEntryView): boolean {
    const track = entry.track;
    if (!entry.trackingIncluded) return false;
    if (!track) return true;
    return !track.localPresent;
  }

  function entrySelectionKey(entry: SourceCollectionEntryView): string {
    return `${entry.position}:${entry.track?.id ?? 'missing'}`;
  }

  function toggleEntrySelection(
    entry: SourceCollectionEntryView,
    selected: boolean,
  ) {
    if (!entry.track || trackingBulkBusy) return;
    const next = new Set(selectedEntryKeys);
    const key = entrySelectionKey(entry);
    if (selected) next.add(key);
    else next.delete(key);
    selectedEntryKeys = next;
  }

  export function toggleSelectAllShown() {
    if (trackingBulkBusy || selectableFilteredEntries.length === 0) return;
    const next = new Set(selectedEntryKeys);
    if (allFilteredSelected) {
      for (const entry of selectableFilteredEntries) {
        next.delete(entrySelectionKey(entry));
      }
    } else {
      for (const entry of selectableFilteredEntries) {
        next.add(entrySelectionKey(entry));
      }
    }
    selectedEntryKeys = next;
  }

  export function clearSelection() {
    if (trackingBulkBusy || selectedEntryKeys.size === 0) return;
    selectedEntryKeys = new Set();
  }

  export async function applySelectedTracking(included: boolean | null) {
    if (selectedEntries.length === 0 || trackingBulkBusy) return;
    if (onSetTrackingMany) {
      await onSetTrackingMany(selectedEntries, included);
    } else if (onSetTracking) {
      await Promise.all(
        selectedEntries.map((entry) => onSetTracking?.(entry, included)),
      );
    }
    selectedEntryKeys = new Set();
  }

  async function copyText(value: string) {
    await navigator.clipboard.writeText(value);
  }

  function trackMenuItems(
    entry: SourceCollectionEntryView,
  ): OverflowMenuItem[] {
    const track = entry.track;
    if (!track) return [];

    const items: OverflowMenuItem[] = [];
    if (track.externalUrl) {
      items.push(
        {
          label: 'Open in Spotify',
          icon: 'link',
          action: () => invoke('open_external_url', { url: track.externalUrl }),
        },
        {
          label: 'Copy Spotify Link',
          icon: 'copy',
          action: () => copyText(track.externalUrl!),
        },
      );
    }

    if (trackingControls) {
      items.push({
        label: entry.trackingIncluded ? 'Exclude from Tracking' : 'Track Song',
        icon: entry.trackingIncluded ? 'close' : 'check',
        action: () => onSetTracking?.(entry, !entry.trackingIncluded),
        disabled: trackingBulkBusy || trackingBusyId === track.id,
      });
      if (entry.trackingOverridden) {
        items.push({
          label: 'Reset to Collection Default',
          icon: 'refresh',
          action: () => onSetTracking?.(entry, null),
          disabled: trackingBulkBusy || trackingBusyId === track.id,
        });
      }
    }

    return items;
  }

  function matchesFilters(entry: SourceCollectionEntryView): boolean {
    const track = entry.track;
    const query = search.trim().toLocaleLowerCase();
    if (!track) {
      if (query) return false;
      if (trackingFilter === 'tracked' && !entry.trackingIncluded) return false;
      if (trackingFilter === 'excluded' && entry.trackingIncluded) return false;
      if (localFilter === 'present') return false;
      if (localFilter === 'attention' && !entry.trackingIncluded) return false;
      if (
        acquisitionFilter !== 'all' ||
        matchFilter !== 'all' ||
        explicitFilter !== 'all'
      )
        return false;
      return true;
    }
    if (
      query &&
      ![
        track.title,
        ...track.artists,
        track.album ?? '',
        track.localFormat ?? '',
      ].some((value) => value.toLocaleLowerCase().includes(query))
    )
      return false;

    if (trackingFilter === 'tracked' && !entry.trackingIncluded) return false;
    if (trackingFilter === 'excluded' && entry.trackingIncluded) return false;
    if (localFilter === 'present' && !track.localPresent) return false;
    if (localFilter === 'missing' && track.localPresent) return false;
    if (localFilter === 'attention' && !needsAttention(entry)) return false;
    if (
      acquisitionFilter !== 'all' &&
      track.acquisitionStatus !== acquisitionFilter
    )
      return false;
    if (matchFilter !== 'all' && track.matchState !== matchFilter) return false;
    if (explicitFilter === 'explicit' && track.explicit !== true) return false;
    if (explicitFilter === 'clean' && track.explicit === true) return false;
    return true;
  }

  const collator = new Intl.Collator(undefined, {
    numeric: true,
    sensitivity: 'base',
  });

  function sortValue(
    entry: SourceCollectionEntryView,
    currentSortId: string,
  ): string | number {
    const track = entry.track;
    switch (currentSortId) {
      case 'title':
        return track?.title ?? '';
      case 'artist':
        return track?.artists.join(', ') ?? '';
      case 'album':
        return track?.album ?? '';
      case 'year':
        return track?.releaseYear ?? -1;
      case 'duration':
        return track?.durationMs ?? -1;
      case 'format':
        return track?.localFormat ?? '';
      case 'local':
        return !track
          ? 3
          : needsAttention(entry)
            ? 2
            : track.localPresent
              ? 0
              : 1;
      case 'tracking':
        return entry.trackingIncluded ? 1 : 0;
      case 'position':
      default:
        return entry.position;
    }
  }

  function compareEntries(
    a: SourceCollectionEntryView,
    b: SourceCollectionEntryView,
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
    trackingFilter = 'all';
    localFilter = 'all';
    acquisitionFilter = 'all';
    matchFilter = 'all';
    explicitFilter = 'all';
  }

  function setLocalFilter(value: 'all' | 'missing' | 'attention') {
    lastAutoLoadOffset = -1;
    localFilter = value;
    loadAllRequested = true;
  }

  function formatNumber(value: number): string {
    return new Intl.NumberFormat().format(value);
  }
</script>

<div class="screen" style="flex:1; height:auto;">
  {#if showFilters}
    <div class="toolbar track-list-toolbar">
      <label class="search-field">
        <Icon name="search" size={17} />
        <input
          bind:value={search}
          class="search-input"
          aria-label="Search tracks"
          placeholder="Search title, artist, album, or playlist"
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
            <span class="filter-label">Local Availability</span>
            <select
              value={localFilter}
              aria-label="Local state"
              class="filter-select"
              onchange={(event) =>
                setLocalFilter(
                  event.currentTarget.value as 'all' | 'missing' | 'attention',
                )}
            >
              <option value="all">Any availability</option>
              <option value="missing">Spotify Only</option>
              <option value="attention">Needs Local Copy</option>
            </select>
          </label>
          <label class="filter-field">
            <span class="filter-label">Tracking</span>
            <select
              bind:value={trackingFilter}
              aria-label="Tracking filter"
              class="filter-select"
            >
              <option value="all">Any state</option>
              <option value="tracked">Tracked</option>
              <option value="excluded">Excluded</option>
            </select>
          </label>
          <label class="filter-field">
            <span class="filter-label">Acquisition</span>
            <select
              bind:value={acquisitionFilter}
              aria-label="Acquisition filter"
              class="filter-select"
            >
              <option value="all">Any state</option>
              {#each acquisitionOptions as status (status)}<option
                  value={status}>{status}</option
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
  {/if}

  {#if loading && entries.length === 0}
    <div class="data-table" aria-label="Loading tracks">
      {#each skeletonRows as row (row)}<div
          class="skeleton"
          style="height:48px; margin-bottom:2px;"
        ></div>{/each}
    </div>
  {:else if entries.length === 0}
    <div class="empty-state">{emptyMessage}</div>
  {:else if filteredEntries.length === 0}
    <div class="empty-state">No tracks match the current filters.</div>
  {:else}
    <div class="data-table" style="display:flex; flex-direction:column;">
      <div
        class="table-scroll"
        style="position:relative; flex:1;"
        bind:clientHeight={viewportHeight}
        onscroll={(event) => (scrollTop = event.currentTarget.scrollTop)}
        aria-label="Track list"
      >
        <DataTableHeader
          bind:columns
          bind:sortId
          bind:sortDirection
          storageKey="refrain.table.spotify.columns.v2"
        />
        <div
          class="relative"
          style={`height:${sortedEntries.length * rowHeight}px; width:${tableWidth}px; min-width:${tableWidth}px;`}
        >
          {#each visibleEntries as entry, visibleIndex (`${entry.position}:${entry.track?.id ?? 'missing'}`)}
            <div
              class="table-row column-table-grid absolute left-0 right-0"
              class:selected={selectedEntryKeys.has(entrySelectionKey(entry))}
              style={`height:${rowHeight}px; transform:translateY(${(startIndex + visibleIndex) * rowHeight}px); grid-template-columns:${gridTemplate}; width:${tableWidth}px; min-width:${tableWidth}px;`}
            >
              {#each columns as column (column.id)}
                {#if column.id === 'position'}
                  {#if trackingControls}
                    <label class="track-selection-cell">
                      <input
                        type="checkbox"
                        checked={selectedEntryKeys.has(
                          entrySelectionKey(entry),
                        )}
                        disabled={!entry.track || trackingBulkBusy}
                        aria-label={entry.track
                          ? `Select ${entry.track.title}`
                          : `Select track ${entry.position + 1}`}
                        onchange={(event) =>
                          toggleEntrySelection(
                            entry,
                            event.currentTarget.checked,
                          )}
                      />
                      <span>{entry.position + 1}</span>
                    </label>
                  {:else}
                    <span class="table-cell-center muted-cell"
                      >{entry.position + 1}</span
                    >
                  {/if}
                {:else if entry.track}
                  {#if column.id === 'title'}
                    <div class="track-primary">
                      {#if entry.track.imageUrl}
                        <img
                          src={entry.track.imageUrl}
                          alt=""
                          loading="lazy"
                          class="artwork-small"
                        />
                      {:else}
                        <div class="artwork-small artwork-fallback">
                          {entry.track.title.slice(0, 1).toUpperCase()}
                        </div>
                      {/if}
                      <div class="track-copy">
                        <div class="track-title-line">
                          <p class="track-title">{entry.track.title}</p>
                          {#if entry.track.explicit}<span class="explicit-badge"
                              >E</span
                            >{/if}
                        </div>
                        <p class="track-secondary">Spotify source</p>
                      </div>
                    </div>
                  {:else if column.id === 'artist'}
                    <span class="cell-truncate"
                      >{entry.track.artists.join(', ') ||
                        'Unknown artist'}</span
                    >
                  {:else if column.id === 'album'}
                    <span class="cell-truncate"
                      >{entry.track.album ?? 'Unknown album'}</span
                    >
                  {:else if column.id === 'year'}
                    <span>{entry.track.releaseYear ?? '—'}</span>
                  {:else if column.id === 'duration'}
                    <span>{formatTrackDuration(entry.track.durationMs)}</span>
                  {:else if column.id === 'format'}
                    <span>{entry.track.localFormat?.toUpperCase() ?? '—'}</span>
                  {:else if column.id === 'local'}
                    <span class="state-text">
                      {#if needsAttention(entry)}
                        <span class="state-dot warning"></span>Needs Local Copy
                      {:else if entry.track.localPresent}
                        <span class="state-dot success"></span>Local
                      {:else}
                        <Icon name="cloud" size={13} />Spotify Only
                      {/if}
                    </span>
                  {:else if column.id === 'tracking'}
                    {#if trackingControls}
                      <div class="tracking-cell">
                        <button
                          type="button"
                          class:success={entry.trackingIncluded}
                          class="chip"
                          aria-pressed={entry.trackingIncluded}
                          title={entry.trackingIncluded
                            ? 'Exclude this song from tracking'
                            : 'Track this song'}
                          onclick={() =>
                            onSetTracking?.(entry, !entry.trackingIncluded)}
                          disabled={trackingBulkBusy ||
                            trackingBusyId === entry.track.id}
                        >
                          {entry.trackingIncluded ? 'Tracked' : 'Excluded'}
                        </button>
                        {#if entry.trackingOverridden}
                          <button
                            type="button"
                            class="link-button"
                            style="font-size:8px;"
                            onclick={() => onSetTracking?.(entry, null)}
                            disabled={trackingBulkBusy ||
                              trackingBusyId === entry.track.id}>Reset</button
                          >
                        {/if}
                      </div>
                    {:else}
                      <span class="chip">Spotify</span>
                    {/if}
                  {:else if column.id === 'actions'}
                    <OverflowMenu
                      items={trackMenuItems(entry)}
                      ariaLabel={`Actions for ${entry.track.title}`}
                    />
                  {/if}
                {:else if column.id === 'title'}
                  <div class="track-primary">
                    <div class="artwork-small artwork-fallback">?</div>
                    <div class="track-copy">
                      <p
                        class="track-title"
                        style="color:var(--text-secondary)"
                      >
                        Unavailable Spotify item
                      </p>
                      <p class="track-secondary">
                        {entry.unavailableReason ?? entry.itemType}
                      </p>
                    </div>
                  </div>
                {:else if column.id === 'actions'}
                  <span class="muted-cell">—</span>
                {:else}
                  <span class="muted-cell">—</span>
                {/if}
              {/each}
            </div>
          {/each}
        </div>
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
    <span
      >Showing {formatNumber(filteredEntries.length)} of {formatNumber(total)} tracks</span
    >
  </footer>
</div>
