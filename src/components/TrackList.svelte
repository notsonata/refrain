<script lang="ts">
  import {
    formatTrackDuration,
    type SourceCollectionEntryView,
  } from '../lib/source';

  export let entries: SourceCollectionEntryView[] = [];
  export let total = 0;
  export let loading = false;
  export let loadingMore = false;
  export let emptyMessage = 'No tracks in this collection.';
  export let trackingControls = false;
  export let trackingBusyId: number | null = null;
  export let showFilters = true;
  export let onLoadMore: (() => void) | undefined = undefined;
  export let onSetTracking:
    | ((entry: SourceCollectionEntryView, included: boolean | null) => void)
    | undefined = undefined;

  let search = '';
  let trackingFilter = 'all';
  let localFilter = 'all';
  let artistFilter = 'all';
  let albumFilter = 'all';
  let yearFilter = 'all';
  let formatFilter = 'all';
  let advancedOpen = false;
  let acquisitionFilter = 'all';
  let matchFilter = 'all';
  let explicitFilter = 'all';
  let durationFilter = 'all';

  const rowHeight = 58;
  const overscan = 8;
  const skeletonRows = Array.from({ length: 8 }, (_, index) => index);
  let scrollTop = 0;
  let viewportHeight = 520;

  $: tracks = entries.flatMap((entry) => (entry.track ? [entry.track] : []));
  $: artistOptions = uniqueSorted(tracks.flatMap((track) => track.artists));
  $: albumOptions = uniqueSorted(
    tracks.flatMap((track) => (track.album ? [track.album] : [])),
  );
  $: yearOptions = uniqueSorted(
    tracks.flatMap((track) =>
      track.releaseYear ? [String(track.releaseYear)] : [],
    ),
  );
  $: formatOptions = uniqueSorted(
    tracks.flatMap((track) => (track.localFormat ? [track.localFormat] : [])),
  );
  $: acquisitionOptions = uniqueSorted(
    tracks.flatMap((track) =>
      track.acquisitionStatus ? [track.acquisitionStatus] : [],
    ),
  );
  $: filteredEntries = entries.filter(matchesFilters);
  $: if (filteredEntries.length === 0) scrollTop = 0;
  $: visibleCount = Math.ceil(viewportHeight / rowHeight) + overscan * 2;
  $: maxStartIndex = Math.max(0, filteredEntries.length - visibleCount);
  $: startIndex = Math.min(
    Math.max(0, Math.floor(scrollTop / rowHeight) - overscan),
    maxStartIndex,
  );
  $: endIndex = Math.min(filteredEntries.length, startIndex + visibleCount);
  $: visibleEntries = filteredEntries.slice(startIndex, endIndex);
  $: hasMore = entries.length < total;

  function uniqueSorted(values: string[]): string[] {
    return [...new Set(values.filter(Boolean))].sort((a, b) =>
      a.localeCompare(b),
    );
  }

  function matchesFilters(entry: SourceCollectionEntryView): boolean {
    const track = entry.track;
    if (!track) return search.trim() === '';
    const query = search.trim().toLocaleLowerCase();
    if (
      query &&
      ![
        track.title,
        ...track.artists,
        track.album ?? '',
        track.localFormat ?? '',
      ].some((value) => value.toLocaleLowerCase().includes(query))
    ) {
      return false;
    }
    if (trackingFilter === 'tracked' && !entry.trackingIncluded) return false;
    if (trackingFilter === 'excluded' && entry.trackingIncluded) return false;
    if (localFilter === 'present' && !track.localPresent) return false;
    if (localFilter === 'missing' && track.localPresent) return false;
    if (artistFilter !== 'all' && !track.artists.includes(artistFilter)) return false;
    if (albumFilter !== 'all' && track.album !== albumFilter) return false;
    if (yearFilter !== 'all' && String(track.releaseYear ?? '') !== yearFilter) return false;
    if (formatFilter !== 'all' && track.localFormat !== formatFilter) return false;
    if (acquisitionFilter !== 'all' && track.acquisitionStatus !== acquisitionFilter) return false;
    if (matchFilter !== 'all' && track.matchState !== matchFilter) return false;
    if (explicitFilter === 'explicit' && track.explicit !== true) return false;
    if (explicitFilter === 'clean' && track.explicit === true) return false;
    if (durationFilter !== 'all') {
      const duration = track.durationMs ?? 0;
      if (durationFilter === 'short' && duration >= 180_000) return false;
      if (
        durationFilter === 'medium' &&
        (duration < 180_000 || duration > 300_000)
      )
        return false;
      if (durationFilter === 'long' && duration <= 300_000) return false;
    }
    return true;
  }

  function clearFilters() {
    search = '';
    trackingFilter = 'all';
    localFilter = 'all';
    artistFilter = 'all';
    albumFilter = 'all';
    yearFilter = 'all';
    formatFilter = 'all';
    acquisitionFilter = 'all';
    matchFilter = 'all';
    explicitFilter = 'all';
    durationFilter = 'all';
  }
</script>

<div class="flex min-h-0 flex-1 flex-col gap-2.5">
  {#if showFilters}
    <div class="shrink-0 rounded-lg border border-slate-800/80 bg-slate-950/45 p-2.5">
      <div class="grid gap-2 md:grid-cols-[minmax(12rem,1.5fr)_repeat(3,minmax(7rem,0.7fr))] xl:grid-cols-[minmax(13rem,1.7fr)_repeat(6,minmax(6.5rem,0.7fr))]">
        <input
          bind:value={search}
          aria-label="Search tracks"
          placeholder="Search title, artist, album…"
          class="min-w-0 rounded-md border border-slate-800 bg-slate-900/70 px-2.5 py-1.5 text-xs text-slate-200 outline-none placeholder:text-slate-600 focus:border-slate-600"
        />
        <select bind:value={trackingFilter} aria-label="Tracking filter" class="filter-select">
          <option value="all">Tracking: all</option>
          <option value="tracked">Tracked</option>
          <option value="excluded">Excluded</option>
        </select>
        <select bind:value={localFilter} aria-label="Local state filter" class="filter-select">
          <option value="all">Local: all</option>
          <option value="present">On disk</option>
          <option value="missing">Missing</option>
        </select>
        <select bind:value={artistFilter} aria-label="Artist filter" class="filter-select">
          <option value="all">Artist: all</option>
          {#each artistOptions as artist (artist)}<option value={artist}>{artist}</option>{/each}
        </select>
        <select bind:value={albumFilter} aria-label="Album filter" class="filter-select">
          <option value="all">Album: all</option>
          {#each albumOptions as album (album)}<option value={album}>{album}</option>{/each}
        </select>
        <select bind:value={yearFilter} aria-label="Year filter" class="filter-select">
          <option value="all">Year: all</option>
          {#each yearOptions as year (year)}<option value={year}>{year}</option>{/each}
        </select>
        <select bind:value={formatFilter} aria-label="Format filter" class="filter-select">
          <option value="all">Format: all</option>
          {#each formatOptions as format (format)}<option value={format}>{format.toUpperCase()}</option>{/each}
        </select>
      </div>
      <div class="mt-2 flex flex-wrap items-center gap-2">
        <button
          type="button"
          onclick={() => (advancedOpen = !advancedOpen)}
          class="rounded-md border border-slate-800 px-2.5 py-1 text-[11px] font-medium text-slate-400 hover:bg-slate-900 hover:text-slate-200"
        >
          {advancedOpen ? 'Hide advanced' : 'Advanced'}
        </button>
        <button type="button" onclick={clearFilters} class="px-1 text-[11px] text-slate-600 hover:text-slate-300">Clear</button>
        <span class="ml-auto text-[11px] text-slate-600">{filteredEntries.length} shown · {entries.length} loaded</span>
      </div>
      {#if advancedOpen}
        <div class="mt-2 grid gap-2 border-t border-slate-900 pt-2 sm:grid-cols-2 xl:grid-cols-4">
          <select bind:value={acquisitionFilter} aria-label="Acquisition filter" class="filter-select">
            <option value="all">Acquisition: all</option>
            {#each acquisitionOptions as status (status)}<option value={status}>{status}</option>{/each}
          </select>
          <select bind:value={matchFilter} aria-label="Match filter" class="filter-select">
            <option value="all">Match: all</option>
            <option value="matched">Matched</option>
            <option value="unmatched">Unmatched</option>
          </select>
          <select bind:value={explicitFilter} aria-label="Explicit filter" class="filter-select">
            <option value="all">Explicit: all</option>
            <option value="explicit">Explicit</option>
            <option value="clean">Not explicit</option>
          </select>
          <select bind:value={durationFilter} aria-label="Duration filter" class="filter-select">
            <option value="all">Duration: all</option>
            <option value="short">Under 3 min</option>
            <option value="medium">3–5 min</option>
            <option value="long">Over 5 min</option>
          </select>
        </div>
      {/if}
    </div>
  {/if}

  {#if loading && entries.length === 0}
    <div class="grid min-h-0 flex-1 content-start gap-1.5 overflow-y-auto" aria-label="Loading tracks">
      {#each skeletonRows as row (row)}<div class="h-[3.6rem] animate-pulse rounded-lg bg-slate-900"></div>{/each}
    </div>
  {:else if entries.length === 0}
    <div class="flex min-h-0 flex-1 items-center justify-center rounded-lg border border-dashed border-slate-800 px-5 py-10 text-center text-xs text-slate-500">
      {emptyMessage}
    </div>
  {:else if filteredEntries.length === 0}
    <div class="flex min-h-0 flex-1 items-center justify-center rounded-lg border border-dashed border-slate-800 px-5 py-10 text-center text-xs text-slate-500">
      No tracks match the current filters.
    </div>
  {:else}
    <div class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border border-slate-800/80 bg-slate-950/35">
      <div
        class="relative min-h-0 flex-1 overflow-y-auto"
        bind:clientHeight={viewportHeight}
        onscroll={(event) => (scrollTop = event.currentTarget.scrollTop)}
        aria-label="Track list"
      >
        <div class="relative" style={`height: ${filteredEntries.length * rowHeight}px`}>
          {#each visibleEntries as entry, visibleIndex (`${entry.position}:${entry.track?.id ?? 'missing'}`)}
            <div
              class="absolute left-0 right-0 flex items-center gap-2.5 border-b border-slate-900 px-2.5 hover:bg-slate-900/40"
              style={`height: ${rowHeight}px; transform: translateY(${(startIndex + visibleIndex) * rowHeight}px)`}
            >
              <span class="w-6 shrink-0 text-center font-mono text-[10px] text-slate-700">{entry.position + 1}</span>
              {#if entry.track}
                {#if entry.track.imageUrl}
                  <img src={entry.track.imageUrl} alt="" loading="lazy" class="h-9 w-9 shrink-0 rounded-md bg-slate-900 object-cover" />
                {:else}
                  <div class="h-9 w-9 shrink-0 rounded-md border border-slate-800 bg-slate-900"></div>
                {/if}
                <div class="min-w-0 flex-1">
                  <div class="flex min-w-0 items-center gap-1.5">
                    <p class="truncate text-xs font-medium text-slate-200">{entry.track.title}</p>
                    {#if entry.track.explicit}<span class="rounded bg-slate-800 px-1 py-0.5 text-[8px] font-semibold text-slate-500">E</span>{/if}
                    {#if entry.track.localPresent}<span class="rounded-full border border-emerald-950 bg-emerald-950/30 px-1.5 py-0.5 text-[8px] font-medium text-emerald-400">Local</span>{/if}
                  </div>
                  <p class="mt-0.5 truncate text-[10px] text-slate-500">
                    {entry.track.artists.join(', ')}
                    {#if entry.track.album}<span class="text-slate-700"> · </span>{entry.track.album}{/if}
                    {#if entry.track.releaseYear}<span class="text-slate-700"> · </span>{entry.track.releaseYear}{/if}
                    {#if entry.track.localFormat}<span class="text-slate-700"> · </span>{entry.track.localFormat.toUpperCase()}{/if}
                  </p>
                </div>
                <span class="hidden shrink-0 font-mono text-[10px] text-slate-600 lg:block">{formatTrackDuration(entry.track.durationMs)}</span>
                {#if trackingControls}
                  <div class="flex w-[6.9rem] shrink-0 items-center justify-end gap-1">
                    {#if entry.trackingOverridden}
                      <button
                        type="button"
                        title="Use collection default"
                        onclick={() => onSetTracking?.(entry, null)}
                        disabled={trackingBusyId === entry.track.id}
                        class="rounded px-1 py-0.5 text-[9px] text-slate-600 hover:bg-slate-800 hover:text-slate-300 disabled:opacity-40"
                      >Reset</button>
                    {/if}
                    <button
                      type="button"
                      onclick={() => onSetTracking?.(entry, !entry.trackingIncluded)}
                      disabled={trackingBusyId === entry.track.id}
                      class={`rounded-full border px-2 py-1 text-[9px] font-semibold transition disabled:opacity-40 ${entry.trackingIncluded ? 'border-emerald-900/80 bg-emerald-950/40 text-emerald-300 hover:bg-emerald-950/70' : 'border-slate-800 text-slate-500 hover:bg-slate-900 hover:text-slate-300'}`}
                    >{entry.trackingIncluded ? 'Tracked' : 'Excluded'}</button>
                  </div>
                {/if}
              {:else}
                <div class="min-w-0 flex-1">
                  <p class="text-xs font-medium text-slate-500">Unavailable Spotify item</p>
                  <p class="mt-0.5 truncate text-[10px] text-slate-700">{entry.unavailableReason ?? entry.itemType}</p>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      </div>

      {#if hasMore}
        <div class="flex shrink-0 justify-center border-t border-slate-900 px-3 py-2">
          <button
            type="button"
            onclick={() => onLoadMore?.()}
            disabled={loadingMore}
            class="rounded-md border border-slate-800 px-2.5 py-1 text-[10px] font-medium text-slate-400 hover:bg-slate-900 hover:text-slate-200 disabled:opacity-50"
          >{loadingMore ? 'Loading…' : `Load more · ${entries.length} of ${total}`}</button>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .filter-select {
    min-width: 0;
    border: 1px solid rgb(30 41 59);
    border-radius: 0.375rem;
    background: rgb(15 23 42 / 0.7);
    padding: 0.375rem 1.6rem 0.375rem 0.625rem;
    font-size: 0.6875rem;
    color: rgb(148 163 184);
    outline: none;
  }
</style>
