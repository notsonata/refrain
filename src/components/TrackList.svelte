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
  export let onLoadMore: (() => void) | undefined = undefined;

  const rowHeight = 68;
  const overscan = 8;
  const skeletonRows = Array.from({ length: 7 }, (_, index) => index);
  let scrollTop = 0;
  let viewportHeight = 520;

  $: if (entries.length === 0) {
    scrollTop = 0;
  }
  $: visibleCount = Math.ceil(viewportHeight / rowHeight) + overscan * 2;
  $: maxStartIndex = Math.max(0, entries.length - visibleCount);
  $: startIndex = Math.min(
    Math.max(0, Math.floor(scrollTop / rowHeight) - overscan),
    maxStartIndex,
  );
  $: endIndex = Math.min(entries.length, startIndex + visibleCount);
  $: visibleEntries = entries.slice(startIndex, endIndex);
  $: hasMore = entries.length < total;
</script>

{#if loading && entries.length === 0}
  <div class="grid gap-2" aria-label="Loading tracks">
    {#each skeletonRows as row (row)}
      <div class="h-16 animate-pulse rounded-lg bg-slate-900"></div>
    {/each}
  </div>
{:else if entries.length === 0}
  <div
    class="rounded-xl border border-dashed border-slate-800 px-6 py-12 text-center text-sm text-slate-500"
  >
    {emptyMessage}
  </div>
{:else}
  <div
    class="overflow-hidden rounded-xl border border-slate-800 bg-slate-950/40"
  >
    <div
      class="grid grid-cols-[3rem_minmax(0,2fr)_minmax(0,1.25fr)_5rem] gap-3 border-b border-slate-800 bg-slate-950 px-4 py-2 text-[11px] font-medium uppercase tracking-wider text-slate-600"
    >
      <span>#</span>
      <span>Track</span>
      <span>Album</span>
      <span class="text-right">Time</span>
    </div>

    <div
      class="relative h-[34rem] overflow-y-auto"
      onscroll={(event) => {
        scrollTop = event.currentTarget.scrollTop;
        viewportHeight = event.currentTarget.clientHeight;
      }}
      aria-label="Track list"
    >
      <div class="relative" style={`height: ${entries.length * rowHeight}px`}>
        {#each visibleEntries as entry, visibleIndex (entry.position)}
          <div
            class="absolute left-0 right-0 grid grid-cols-[3rem_minmax(0,2fr)_minmax(0,1.25fr)_5rem] items-center gap-3 border-b border-slate-900 px-4 text-sm hover:bg-slate-900/50"
            style={`height: ${rowHeight}px; transform: translateY(${(startIndex + visibleIndex) * rowHeight}px)`}
          >
            <span class="font-mono text-xs text-slate-600">
              {entry.position + 1}
            </span>

            {#if entry.track}
              <div class="flex min-w-0 items-center gap-3">
                {#if entry.track.imageUrl}
                  <img
                    src={entry.track.imageUrl}
                    alt=""
                    loading="lazy"
                    class="h-10 w-10 shrink-0 rounded bg-slate-900 object-cover"
                  />
                {:else}
                  <div class="h-10 w-10 shrink-0 rounded bg-slate-900"></div>
                {/if}
                <div class="min-w-0">
                  <div class="flex items-center gap-2">
                    <span class="truncate font-medium text-slate-200">
                      {entry.track.title}
                    </span>
                    {#if entry.track.explicit}
                      <span
                        class="rounded bg-slate-800 px-1.5 py-0.5 text-[10px] font-semibold uppercase text-slate-400"
                      >
                        E
                      </span>
                    {/if}
                  </div>
                  <p class="mt-0.5 truncate text-xs text-slate-500">
                    {entry.track.artists.join(', ')}
                  </p>
                </div>
              </div>
              <span class="truncate text-xs text-slate-500">
                {entry.track.album ?? '—'}
              </span>
              <span class="text-right font-mono text-xs text-slate-500">
                {formatTrackDuration(entry.track.durationMs)}
              </span>
            {:else}
              <div class="min-w-0">
                <p class="font-medium text-slate-400">
                  Unavailable Spotify item
                </p>
                <p class="mt-0.5 truncate text-xs text-slate-600">
                  {entry.unavailableReason ?? entry.itemType}
                </p>
              </div>
              <span class="text-xs text-slate-700">—</span>
              <span class="text-right text-xs text-slate-700">—</span>
            {/if}
          </div>
        {/each}
      </div>
    </div>

    <div
      class="flex items-center justify-between border-t border-slate-800 px-4 py-3 text-xs text-slate-500"
    >
      <span>{entries.length} of {total} entries loaded</span>
      {#if hasMore}
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
