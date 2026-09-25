<script lang="ts">
  import type { LibraryTrackRow } from '../lib/library';
  import { formatTrackDuration } from '../lib/source';

  export let tracks: LibraryTrackRow[] = [];
  export let total = 0;
  export let loading = false;
  export let loadingMore = false;
  export let error: string | null = null;
  export let onLoadMore: (() => void) | undefined = undefined;

  function stateLabel(track: LibraryTrackRow): string {
    if (track.preferredFile) return 'Available';
    if (track.presentFileCount > 0) return 'Present';
    if (track.invalidFileCount > 0) return 'Invalid';
    return 'Missing';
  }

  function stateClass(track: LibraryTrackRow): string {
    if (track.preferredFile || track.presentFileCount > 0) {
      return 'bg-emerald-950 text-emerald-300';
    }
    if (track.invalidFileCount > 0) {
      return 'bg-rose-950 text-rose-300';
    }
    return 'bg-amber-950 text-amber-300';
  }
</script>

<div class="min-w-0">
  <div class="mb-5 flex items-end justify-between gap-4">
    <div>
      <p class="text-xs uppercase tracking-wider text-slate-600">
        Local library
      </p>
      <h2 class="mt-1 text-2xl font-semibold">Library</h2>
      <p class="mt-1 text-sm text-slate-500">
        {total} logical {total === 1 ? 'track' : 'tracks'} indexed
      </p>
    </div>
  </div>

  {#if error}
    <div
      class="rounded-xl border border-amber-900 bg-amber-950/30 p-5 text-sm text-amber-200"
    >
      {error}
    </div>
  {:else if loading && tracks.length === 0}
    <div class="grid gap-2" aria-label="Loading library tracks">
      {#each Array.from({ length: 7 }) as index (index)}
        <div class="h-16 animate-pulse rounded-lg bg-slate-900"></div>
      {/each}
    </div>
  {:else if tracks.length === 0}
    <div
      class="rounded-xl border border-dashed border-slate-800 px-6 py-14 text-center"
    >
      <p class="text-sm font-medium text-slate-300">
        No logical library tracks yet.
      </p>
      <p class="mt-2 text-sm text-slate-500">
        Scan the local library and run synchronization to build the library
        index.
      </p>
    </div>
  {:else}
    <div
      class="overflow-hidden rounded-xl border border-slate-800 bg-slate-950/40"
    >
      <div
        class="grid grid-cols-[minmax(0,2fr)_minmax(0,1.25fr)_7rem_minmax(10rem,1.25fr)] gap-3 border-b border-slate-800 bg-slate-950 px-4 py-2 text-[11px] font-medium uppercase tracking-wider text-slate-600"
      >
        <span>Track</span>
        <span>Album</span>
        <span>Status</span>
        <span>Local file</span>
      </div>

      <div class="max-h-[42rem] overflow-y-auto">
        {#each tracks as track (track.id)}
          <div
            class="grid min-h-16 grid-cols-[minmax(0,2fr)_minmax(0,1.25fr)_7rem_minmax(10rem,1.25fr)] items-center gap-3 border-b border-slate-900 px-4 py-3 text-sm last:border-b-0 hover:bg-slate-900/40"
          >
            <div class="min-w-0">
              <p class="truncate font-medium text-slate-200">{track.title}</p>
              <p class="mt-0.5 truncate text-xs text-slate-500">
                {track.artists.join(', ') || 'Unknown artist'} ·
                {formatTrackDuration(track.durationMs)} · {track.sourceTrackCount}
                source {track.sourceTrackCount === 1 ? 'track' : 'tracks'}
              </p>
            </div>
            <div class="min-w-0 text-xs text-slate-500">
              <p class="truncate">{track.album ?? '—'}</p>
              {#if track.releaseYear}
                <p class="mt-0.5 text-slate-700">{track.releaseYear}</p>
              {/if}
            </div>
            <div>
              <span
                class={`rounded px-2 py-1 text-[10px] font-medium uppercase tracking-wide ${stateClass(track)}`}
              >
                {stateLabel(track)}
              </span>
            </div>
            <div class="min-w-0">
              {#if track.preferredFile}
                <p
                  class="truncate font-mono text-xs text-slate-400"
                  title={track.preferredFile.path}
                >
                  {track.preferredFile.path}
                </p>
                <p class="mt-0.5 text-[11px] text-slate-700">
                  {track.preferredFile.ownership} · {track.preferredFile
                    .format ?? 'audio'}
                </p>
              {:else}
                <p class="text-xs text-slate-600">
                  {track.presentFileCount} present · {track.missingFileCount} missing
                  ·
                  {track.invalidFileCount} invalid
                </p>
              {/if}
            </div>
          </div>
        {/each}
      </div>

      <div
        class="flex items-center justify-between border-t border-slate-800 px-4 py-3 text-xs text-slate-500"
      >
        <span>{tracks.length} of {total} tracks loaded</span>
        {#if tracks.length < total}
          <button
            type="button"
            onclick={() => onLoadMore?.()}
            disabled={loadingMore}
            class="rounded-md border border-slate-700 px-3 py-1.5 font-medium text-slate-300 transition hover:bg-slate-900 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {loadingMore ? 'Loading…' : 'Load more'}
          </button>
        {/if}
      </div>
    </div>
  {/if}
</div>
