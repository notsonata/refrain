<script lang="ts">
  import type {
    IssueCounts,
    IssueKind,
    IssueRow,
    MatchReview,
    MatchReviewCandidate,
  } from '../lib/issues';
  import { formatTrackDuration } from '../lib/source';

  export let issues: IssueRow[] = [];
  export let total = 0;
  export let counts: IssueCounts = {
    matchReview: 0,
    missingLocalFile: 0,
    inaccessibleCollection: 0,
    invalidLocalFile: 0,
    acquisitionFailed: 0,
  };
  export let selectedIssueId: string | null = null;
  export let review: MatchReview | null = null;
  export let loading = false;
  export let loadingMore = false;
  export let reviewLoading = false;
  export let decisionBusy = false;
  export let error: string | null = null;
  export let onSelect: ((issue: IssueRow) => void) | undefined = undefined;
  export let onLoadMore: (() => void) | undefined = undefined;
  export let onConfirm:
    ((sourceTrackId: number, libraryTrackId: number) => void) | undefined =
    undefined;
  export let onReject:
    ((sourceTrackId: number, libraryTrackId: number) => void) | undefined =
    undefined;
  export let onClearRejection:
    ((sourceTrackId: number, libraryTrackId: number) => void) | undefined =
    undefined;

  $: selectedIssue =
    issues.find((issue) => issue.id === selectedIssueId) ?? issues[0] ?? null;

  function issueLabel(kind: IssueKind): string {
    switch (kind) {
      case 'matchReview':
        return 'Needs review';
      case 'missingLocalFile':
        return 'Missing';
      case 'inaccessibleCollection':
        return 'Inaccessible';
      case 'invalidLocalFile':
        return 'Invalid file';
      case 'acquisitionFailed':
        return 'Acquisition failed';
    }
  }

  function issueBadgeClass(kind: IssueKind): string {
    switch (kind) {
      case 'matchReview':
        return 'bg-violet-950 text-violet-300';
      case 'missingLocalFile':
      case 'inaccessibleCollection':
        return 'bg-amber-950 text-amber-300';
      case 'invalidLocalFile':
      case 'acquisitionFailed':
        return 'bg-rose-950 text-rose-300';
    }
  }

  function score(candidate: MatchReviewCandidate): string {
    return `${Math.round(candidate.evidence.score.total / 100)}%`;
  }

  function isRejected(candidate: MatchReviewCandidate): boolean {
    return (
      review?.rejectedLibraryTrackIds.includes(candidate.track.id) ?? false
    );
  }
</script>

<div class="flex h-full min-h-0 min-w-0 flex-col overflow-hidden">
  <div class="mb-3 flex flex-wrap items-end justify-between gap-3">
    <div>
      <p class="text-xs uppercase tracking-wider text-slate-600">
        Resolution queue
      </p>
      <h2 class="mt-0.5 text-lg font-semibold">Issues</h2>
      <p class="mt-0.5 text-xs text-slate-500">
        {total} unresolved {total === 1 ? 'condition' : 'conditions'}
      </p>
    </div>
    <div class="flex flex-wrap gap-2 text-[11px] text-slate-500">
      <span class="rounded bg-slate-900 px-2 py-1"
        >{counts.matchReview} review</span
      >
      <span class="rounded bg-slate-900 px-2 py-1"
        >{counts.missingLocalFile} missing</span
      >
      <span class="rounded bg-slate-900 px-2 py-1"
        >{counts.invalidLocalFile} invalid</span
      >
      <span class="rounded bg-slate-900 px-2 py-1"
        >{counts.acquisitionFailed} acquisition</span
      >
    </div>
  </div>

  {#if error}
    <div
      class="mb-3 rounded-lg border border-amber-900 bg-amber-950/30 p-3 text-xs text-amber-200"
    >
      {error}
    </div>
  {/if}

  {#if loading && issues.length === 0}
    <div class="grid gap-2" aria-label="Loading issues">
      {#each Array.from({ length: 6 }, (_, index) => index) as index (index)}
        <div class="h-16 animate-pulse rounded-lg bg-slate-900"></div>
      {/each}
    </div>
  {:else if issues.length === 0}
    <div
      class="rounded-lg border border-dashed border-emerald-900/60 px-5 py-10 text-center"
    >
      <p class="text-sm font-medium text-emerald-300">No unresolved issues.</p>
      <p class="mt-2 text-sm text-slate-500">
        Refrain will repopulate this view when durable source or library state
        needs attention.
      </p>
    </div>
  {:else}
    <div
      class="grid min-h-0 min-w-0 flex-1 grid-cols-[12rem_minmax(0,1fr)] gap-3 overflow-hidden"
    >
      <aside
        class="flex min-h-0 flex-col overflow-hidden rounded-lg border border-slate-800 bg-slate-950/40"
      >
        <div class="min-h-0 flex-1 overflow-y-auto p-2">
          {#each issues as issue (issue.id)}
            <button
              type="button"
              onclick={() => onSelect?.(issue)}
              class={`mb-1 w-full rounded-md px-2.5 py-2 text-left transition ${selectedIssue?.id === issue.id ? 'bg-slate-900' : 'hover:bg-slate-900/60'}`}
            >
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <p class="truncate text-xs font-medium text-slate-200">
                    {issue.title}
                  </p>
                  <p class="mt-0.5 truncate text-xs text-slate-600">
                    {issue.subtitle ?? issue.detail ?? 'Needs attention'}
                  </p>
                </div>
                <span
                  class={`shrink-0 rounded px-1.5 py-0.5 text-[9px] font-medium uppercase tracking-wide ${issueBadgeClass(issue.kind)}`}
                >
                  {issueLabel(issue.kind)}
                </span>
              </div>
            </button>
          {/each}
        </div>
        {#if issues.length < total}
          <div class="border-t border-slate-800 p-2">
            <button
              type="button"
              onclick={() => onLoadMore?.()}
              disabled={loadingMore}
              class="w-full rounded-lg border border-slate-800 px-3 py-2 text-xs font-medium text-slate-400 hover:bg-slate-900 disabled:opacity-50"
            >
              {loadingMore
                ? 'Loading…'
                : `Load more (${issues.length} of ${total})`}
            </button>
          </div>
        {/if}
      </aside>

      <section
        class="min-h-0 min-w-0 overflow-y-auto rounded-lg border border-slate-800 p-4"
      >
        {#if selectedIssue}
          <div class="flex flex-wrap items-start justify-between gap-3">
            <div class="min-w-0">
              <span
                class={`rounded px-2 py-1 text-[10px] font-medium uppercase tracking-wide ${issueBadgeClass(selectedIssue.kind)}`}
              >
                {issueLabel(selectedIssue.kind)}
              </span>
              <h3 class="mt-2 truncate text-base font-semibold">
                {selectedIssue.title}
              </h3>
              {#if selectedIssue.subtitle}
                <p class="mt-1 text-xs text-slate-500">
                  {selectedIssue.subtitle}
                </p>
              {/if}
            </div>
          </div>

          {#if selectedIssue.kind === 'matchReview'}
            {#if reviewLoading}
              <div
                class="mt-6 h-48 animate-pulse rounded-lg bg-slate-900"
              ></div>
            {:else if review}
              <div
                class="mt-4 rounded-lg border border-slate-800 bg-slate-900/40 p-3"
              >
                <p
                  class="text-xs font-medium uppercase tracking-wider text-slate-600"
                >
                  Spotify source
                </p>
                <p class="mt-2 font-medium text-slate-200">
                  {review.source.title}
                </p>
                <p class="mt-1 text-xs text-slate-500">
                  {review.source.artists.join(', ') || 'Unknown artist'} ·
                  {review.source.album ?? 'Unknown album'} ·
                  {formatTrackDuration(review.source.durationMs)}
                </p>
                {#if review.source.versionKind || review.source.versionDetail}
                  <p class="mt-2 text-xs text-amber-300/80">
                    {[review.source.versionKind, review.source.versionDetail]
                      .filter(Boolean)
                      .join(' · ')}
                  </p>
                {/if}
              </div>

              <div class="mt-4 grid gap-2">
                {#each review.candidates as candidate (candidate.track.id)}
                  <article
                    class={`rounded-lg border p-3 ${isRejected(candidate) ? 'border-slate-800 bg-slate-950/60 opacity-70' : 'border-slate-800 bg-slate-900/25'}`}
                  >
                    <div
                      class="flex flex-wrap items-start justify-between gap-3"
                    >
                      <div class="min-w-0">
                        <div class="flex flex-wrap items-center gap-2">
                          <h4 class="font-medium text-slate-200">
                            {candidate.track.title}
                          </h4>
                          <span class="font-mono text-xs text-slate-500"
                            >{score(candidate)}</span
                          >
                          {#if candidate.evidence.exactIsrc}
                            <span
                              class="rounded bg-emerald-950 px-1.5 py-0.5 text-[10px] text-emerald-300"
                            >
                              ISRC
                            </span>
                          {/if}
                          {#if isRejected(candidate)}
                            <span
                              class="rounded bg-slate-800 px-1.5 py-0.5 text-[10px] text-slate-400"
                            >
                              rejected
                            </span>
                          {/if}
                        </div>
                        <p class="mt-1 text-xs text-slate-500">
                          {candidate.track.artists.join(', ') ||
                            'Unknown artist'} ·
                          {candidate.track.album ?? 'Unknown album'} ·
                          {formatTrackDuration(candidate.track.durationMs)}
                        </p>
                      </div>
                      <div class="flex shrink-0 gap-2">
                        {#if isRejected(candidate)}
                          <button
                            type="button"
                            onclick={() =>
                              onClearRejection?.(
                                review.source.id,
                                candidate.track.id,
                              )}
                            disabled={decisionBusy}
                            class="rounded-md border border-slate-700 px-3 py-1.5 text-xs font-medium text-slate-300 hover:bg-slate-800 disabled:opacity-50"
                          >
                            Clear rejection
                          </button>
                        {:else}
                          <button
                            type="button"
                            onclick={() =>
                              onReject?.(review.source.id, candidate.track.id)}
                            disabled={decisionBusy}
                            class="rounded-md border border-slate-700 px-3 py-1.5 text-xs font-medium text-slate-300 hover:bg-slate-800 disabled:opacity-50"
                          >
                            Reject
                          </button>
                          <button
                            type="button"
                            onclick={() =>
                              onConfirm?.(review.source.id, candidate.track.id)}
                            disabled={decisionBusy}
                            class="rounded-md bg-slate-100 px-3 py-1.5 text-xs font-medium text-slate-950 hover:bg-white disabled:opacity-50"
                          >
                            Confirm match
                          </button>
                        {/if}
                      </div>
                    </div>

                    {#if candidate.evidence.warnings.length > 0 || candidate.evidence.incompatibilities.length > 0}
                      <p class="mt-3 text-xs leading-5 text-amber-300/70">
                        {[
                          ...candidate.evidence.warnings,
                          ...candidate.evidence.incompatibilities,
                        ].join(' · ')}
                      </p>
                    {/if}

                    <div class="mt-3 grid gap-1.5">
                      {#if candidate.files.length === 0}
                        <p class="text-xs text-slate-700">
                          No local files linked to this candidate.
                        </p>
                      {:else}
                        {#each candidate.files as file (file.id)}
                          <div class="flex min-w-0 items-center gap-2 text-xs">
                            <span
                              class={`shrink-0 rounded px-1.5 py-0.5 text-[9px] uppercase ${file.state === 'present' ? 'bg-emerald-950 text-emerald-300' : file.state === 'invalid' ? 'bg-rose-950 text-rose-300' : 'bg-amber-950 text-amber-300'}`}
                            >
                              {file.state}
                            </span>
                            <span
                              class="truncate font-mono text-slate-500"
                              title={file.path}>{file.path}</span
                            >
                            {#if file.isPreferred}
                              <span class="shrink-0 text-slate-700"
                                >preferred</span
                              >
                            {/if}
                          </div>
                        {/each}
                      {/if}
                    </div>
                  </article>
                {/each}
              </div>
            {:else}
              <p class="mt-6 text-sm text-slate-500">
                Match details are no longer available. Refresh the issue list.
              </p>
            {/if}
          {:else}
            {#if selectedIssue.detail}
              <p class="mt-5 max-w-2xl text-sm leading-6 text-slate-400">
                {selectedIssue.detail}
              </p>
            {/if}
            {#if selectedIssue.path}
              <div
                class="mt-5 rounded-lg border border-slate-800 bg-slate-900/40 p-4"
              >
                <p
                  class="text-xs font-medium uppercase tracking-wider text-slate-600"
                >
                  Path
                </p>
                <p
                  class="mt-2 break-all font-mono text-xs leading-5 text-slate-400"
                >
                  {selectedIssue.path}
                </p>
              </div>
            {/if}
            <p class="mt-5 text-xs leading-5 text-slate-600">
              This issue is derived from current state and disappears
              automatically after the underlying condition is resolved and
              Refrain refreshes that state.
            </p>
          {/if}
        {/if}
      </section>
    </div>
  {/if}
</div>
