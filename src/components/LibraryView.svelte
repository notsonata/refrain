<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { LibraryTrackRow } from '../lib/library';
  import type { SyncRun } from '../lib/sync';
  import { formatTrackDuration } from '../lib/source';

  export let tracks: LibraryTrackRow[] = [];
  export let total = 0;
  export let loading = false;
  export let loadingMore = false;
  export let error: string | null = null;
  export let onLoadMore: (() => void) | undefined = undefined;
  export let onSynchronize: (() => void) | undefined = undefined;
  export let onShowIssues: (() => void) | undefined = undefined;
  export let syncBusy = false;
  export let syncRun: SyncRun | null = null;
  export let indexedFiles = 0;
  export let issueCount = 0;

  let search = '';
  let spotifyState = 'all';
  let artistFilter = 'all';
  let albumFilter = 'all';
  let yearFilter = 'all';
  let formatFilter = 'all';
  let membershipFilter = 'all';
  let advancedOpen = false;
  let pathFilter = '';
  let acquisitionFilter = 'all';
  let matchFilter = 'all';
  let explicitFilter = 'all';
  let durationFilter = 'all';

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
    tracks.flatMap((track) =>
      track.preferredFile?.format ? [track.preferredFile.format] : [],
    ),
  );
  $: membershipOptions = uniqueSorted(
    tracks.flatMap((track) =>
      track.spotifyMemberships.map((membership) => membershipLabel(membership.kind, membership.name)),
    ),
  );
  $: acquisitionOptions = uniqueSorted(
    tracks.flatMap((track) =>
      track.acquisitionStatus ? [track.acquisitionStatus] : [],
    ),
  );
  $: filteredTracks = tracks.filter(matchesFilters);

  function uniqueSorted(values: string[]): string[] {
    return [...new Set(values.filter(Boolean))].sort((a, b) => a.localeCompare(b));
  }

  function membershipLabel(kind: string, name: string): string {
    if (kind === 'liked_songs') return 'Liked Songs';
    return name;
  }

  function artworkUrl(track: LibraryTrackRow): string | null {
    const path = track.preferredFile?.artworkPath;
    if (!path) return null;
    return convertFileSrc(path);
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
    ) {
      return false;
    }

    const onSpotify = track.spotifyMemberships.length > 0;
    if (spotifyState === 'spotify' && !onSpotify) return false;
    if (spotifyState === 'local' && onSpotify) return false;
    if (artistFilter !== 'all' && !track.artists.includes(artistFilter)) return false;
    if (albumFilter !== 'all' && track.album !== albumFilter) return false;
    if (yearFilter !== 'all' && String(track.releaseYear ?? '') !== yearFilter) return false;
    if (formatFilter !== 'all' && track.preferredFile?.format !== formatFilter) return false;
    if (
      membershipFilter !== 'all' &&
      !track.spotifyMemberships.some(
        (membership) => membershipLabel(membership.kind, membership.name) === membershipFilter,
      )
    ) {
      return false;
    }

    const normalizedPath = pathFilter.trim().toLocaleLowerCase();
    if (normalizedPath && !filePath.toLocaleLowerCase().includes(normalizedPath)) return false;
    if (acquisitionFilter !== 'all' && track.acquisitionStatus !== acquisitionFilter) return false;
    if (matchFilter === 'matched' && track.sourceTrackCount <= 0) return false;
    if (matchFilter === 'unmatched' && track.sourceTrackCount > 0) return false;
    if (explicitFilter === 'explicit' && track.explicit !== true) return false;
    if (explicitFilter === 'clean' && track.explicit === true) return false;

    if (durationFilter !== 'all') {
      const duration = track.durationMs ?? 0;
      if (durationFilter === 'short' && duration >= 180_000) return false;
      if (durationFilter === 'medium' && (duration < 180_000 || duration > 300_000)) return false;
      if (durationFilter === 'long' && duration <= 300_000) return false;
    }

    return true;
  }

  function clearFilters() {
    search = '';
    spotifyState = 'all';
    artistFilter = 'all';
    albumFilter = 'all';
    yearFilter = 'all';
    formatFilter = 'all';
    membershipFilter = 'all';
    pathFilter = '';
    acquisitionFilter = 'all';
    matchFilter = 'all';
    explicitFilter = 'all';
    durationFilter = 'all';
  }
</script>

<div class="flex h-full min-h-0 flex-col">
  <div class="mb-3 flex shrink-0 items-start justify-between gap-4">
    <div class="min-w-0">
      <p class="text-[10px] font-medium uppercase tracking-[0.14em] text-slate-600">Local</p>
      <h2 class="mt-0.5 text-lg font-semibold tracking-tight text-slate-100">Your library</h2>
      <p class="mt-0.5 max-w-2xl text-xs text-slate-500">
        Music actually on disk, with matched Spotify membership as context.
      </p>
    </div>
    <div class="flex shrink-0 items-center gap-1.5">
      {#if issueCount > 0}
        <button
          type="button"
          onclick={() => onShowIssues?.()}
          class="rounded-md border border-slate-800 px-2.5 py-1.5 text-[11px] font-medium text-slate-400 hover:bg-slate-900 hover:text-slate-200"
        >
          {issueCount} need attention
        </button>
      {/if}
      <button
        type="button"
        onclick={() => onSynchronize?.()}
        disabled={syncBusy}
        class="rounded-md bg-slate-100 px-3 py-1.5 text-xs font-semibold text-slate-950 hover:bg-white disabled:cursor-not-allowed disabled:opacity-50"
      >
        {syncBusy ? 'Syncing…' : 'Sync local'}
      </button>
    </div>
  </div>

  <div class="mb-2.5 grid shrink-0 grid-cols-3 gap-1.5">
    <div class="rounded-lg border border-slate-800/80 bg-slate-900/30 px-3 py-2">
      <p class="text-[9px] uppercase tracking-wider text-slate-600">On disk</p>
      <p class="mt-0.5 text-base font-semibold text-slate-200">{total}</p>
    </div>
    <div class="rounded-lg border border-slate-800/80 bg-slate-900/30 px-3 py-2">
      <p class="text-[9px] uppercase tracking-wider text-slate-600">Indexed files</p>
      <p class="mt-0.5 text-base font-semibold text-slate-200">{indexedFiles}</p>
    </div>
    <div class="rounded-lg border border-slate-800/80 bg-slate-900/30 px-3 py-2">
      <p class="text-[9px] uppercase tracking-wider text-slate-600">Last local sync</p>
      <p class="mt-0.5 truncate text-xs font-medium text-slate-300">{syncRun ? syncRun.status : 'Not run yet'}</p>
      {#if syncRun}
        <p class="mt-0.5 text-[9px] text-slate-600">{syncRun.matched} matched · {syncRun.needsReview} review</p>
      {/if}
    </div>
  </div>

  <div class="mb-2.5 shrink-0 rounded-lg border border-slate-800/80 bg-slate-950/45 p-2.5">
    <div class="grid gap-2 md:grid-cols-[minmax(12rem,1.5fr)_repeat(3,minmax(7rem,0.75fr))] xl:grid-cols-[minmax(12rem,1.7fr)_repeat(6,minmax(6.5rem,0.7fr))]">
      <input
        bind:value={search}
        aria-label="Search local library"
        placeholder="Search title, artist, album, path…"
        class="filter-input"
      />
      <select bind:value={spotifyState} aria-label="Spotify state filter" class="filter-select">
        <option value="all">Spotify: all</option>
        <option value="spotify">On Spotify</option>
        <option value="local">Local only</option>
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
      <select bind:value={membershipFilter} aria-label="Spotify membership filter" class="filter-select">
        <option value="all">Membership: all</option>
        {#each membershipOptions as membership (membership)}<option value={membership}>{membership}</option>{/each}
      </select>
    </div>
    <div class="mt-2 flex items-center gap-2">
      <button
        type="button"
        onclick={() => (advancedOpen = !advancedOpen)}
        class="rounded-md border border-slate-800 px-2.5 py-1 text-[10px] font-medium text-slate-400 hover:bg-slate-900 hover:text-slate-200"
      >
        {advancedOpen ? 'Hide advanced' : 'Advanced'}
      </button>
      <button type="button" onclick={clearFilters} class="text-[10px] text-slate-600 hover:text-slate-300">Clear</button>
      <span class="ml-auto text-[10px] text-slate-600">{filteredTracks.length} shown · {tracks.length} loaded</span>
    </div>
    {#if advancedOpen}
      <div class="mt-2 grid gap-2 border-t border-slate-900 pt-2 md:grid-cols-2 xl:grid-cols-5">
        <input bind:value={pathFilter} aria-label="File path filter" placeholder="Path contains…" class="filter-input" />
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

  {#if error}
    <div class="rounded-lg border border-amber-900 bg-amber-950/30 p-4 text-xs text-amber-200">{error}</div>
  {:else if loading && tracks.length === 0}
    <div class="grid gap-1.5" aria-label="Loading local library">
      {#each Array.from({ length: 8 }, (_, index) => index) as index (index)}
        <div class="h-[3.65rem] animate-pulse rounded-lg bg-slate-900"></div>
      {/each}
    </div>
  {:else if tracks.length === 0}
    <div class="flex min-h-0 flex-1 items-center justify-center rounded-lg border border-dashed border-slate-800 px-5 py-10 text-center">
      <div>
        <p class="text-xs font-medium text-slate-300">No local tracks indexed yet.</p>
        <p class="mt-1 text-xs text-slate-500">Choose your library folder in Settings, then run Local Sync.</p>
      </div>
    </div>
  {:else if filteredTracks.length === 0}
    <div class="flex min-h-0 flex-1 items-center justify-center rounded-lg border border-dashed border-slate-800 px-5 py-10 text-center text-xs text-slate-500">
      No local tracks match the current filters.
    </div>
  {:else}
    <div class="min-h-0 flex-1 overflow-y-auto rounded-lg border border-slate-800/80 bg-slate-950/35">
      {#each filteredTracks as track (track.id)}
        {@const art = artworkUrl(track)}
        <article class="flex min-h-[3.65rem] items-center gap-2.5 border-b border-slate-900 px-2.5 py-1.5 last:border-b-0 hover:bg-slate-900/35">
          {#if art}
            <img src={art} alt="" loading="lazy" class="h-9 w-9 shrink-0 rounded-md bg-slate-900 object-cover" />
          {:else}
            <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-md border border-slate-800 bg-slate-900 text-[10px] font-semibold text-slate-500">
              {(track.artists[0] ?? track.title).slice(0, 1).toUpperCase()}
            </div>
          {/if}
          <div class="min-w-0 flex-1">
            <div class="flex min-w-0 items-center gap-1.5">
              <p class="truncate text-xs font-medium text-slate-200">{track.title}</p>
              {#if track.explicit}<span class="rounded bg-slate-800 px-1 py-0.5 text-[8px] font-semibold text-slate-500">E</span>{/if}
              {#if track.spotifyMemberships.length === 0}
                <span class="shrink-0 rounded-full border border-slate-800 px-1.5 py-0.5 text-[8px] font-medium text-slate-500">Local only</span>
              {:else}
                <span class="shrink-0 rounded-full border border-emerald-900/70 bg-emerald-950/35 px-1.5 py-0.5 text-[8px] font-medium text-emerald-300">On Spotify</span>
              {/if}
            </div>
            <p class="mt-0.5 truncate text-[10px] text-slate-500">
              {track.artists.join(', ') || 'Unknown artist'}
              <span class="text-slate-700"> · </span>{track.album ?? 'Unknown album'}
              {#if track.releaseYear}<span class="text-slate-700"> · </span>{track.releaseYear}{/if}
              <span class="text-slate-700"> · </span>{formatTrackDuration(track.durationMs)}
              {#if track.preferredFile?.format}<span class="text-slate-700"> · </span>{track.preferredFile.format.toUpperCase()}{/if}
            </p>
            {#if track.preferredFile}
              <p class="mt-0.5 truncate font-mono text-[9px] text-slate-700" title={track.preferredFile.path}>{track.preferredFile.path}</p>
            {/if}
          </div>
          {#if track.spotifyMemberships.length > 0}
            <div class="hidden max-w-[16rem] flex-wrap justify-end gap-1 2xl:flex">
              {#each track.spotifyMemberships.slice(0, 3) as membership (`${membership.kind}:${membership.name}`)}
                <span class="rounded bg-slate-900 px-1.5 py-0.5 text-[8px] text-slate-500">{membershipLabel(membership.kind, membership.name)}</span>
              {/each}
              {#if track.spotifyMemberships.length > 3}<span class="px-1 py-0.5 text-[8px] text-slate-700">+{track.spotifyMemberships.length - 3}</span>{/if}
            </div>
          {/if}
        </article>
      {/each}
      {#if tracks.length < total}
        <div class="flex justify-center border-t border-slate-900 px-3 py-2">
          <button
            type="button"
            onclick={() => onLoadMore?.()}
            disabled={loadingMore}
            class="rounded-md border border-slate-800 px-2.5 py-1 text-[10px] font-medium text-slate-400 hover:bg-slate-900 hover:text-slate-200 disabled:opacity-50"
          >
            {loadingMore ? 'Loading…' : `Load more · ${tracks.length} of ${total}`}
          </button>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .filter-input,
  .filter-select {
    min-width: 0;
    border: 1px solid rgb(30 41 59);
    border-radius: 0.375rem;
    background: rgb(15 23 42 / 0.7);
    padding: 0.375rem 0.625rem;
    font-size: 0.6875rem;
    color: rgb(148 163 184);
    outline: none;
  }

  .filter-select {
    padding-right: 1.6rem;
  }

  .filter-input::placeholder {
    color: rgb(71 85 105);
  }
</style>
