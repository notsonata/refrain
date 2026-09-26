<script lang="ts">
  import Icon from './Icon.svelte';
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
    localOnlyTrack: 0,
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

  let search = '';
  let kindFilter: 'all' | 'localOnly' | 'trackedMissing' = 'all';

  $: filteredIssues = issues.filter((issue) => {
    const query = search.trim().toLocaleLowerCase();
    if (
      query &&
      ![
        issue.title,
        issue.subtitle ?? '',
        issue.detail ?? '',
        issue.path ?? '',
      ].some((value) => value.toLocaleLowerCase().includes(query))
    )
      return false;
    if (kindFilter === 'localOnly') return issue.kind === 'localOnlyTrack';
    if (kindFilter === 'trackedMissing') {
      return issue.kind === 'missingLocalFile' || issue.kind === 'matchReview';
    }
    return true;
  });
  $: selectedIssue =
    filteredIssues.find((issue) => issue.id === selectedIssueId) ?? null;

  function issueLabel(kind: IssueKind): string {
    switch (kind) {
      case 'matchReview':
        return 'Needs Local Copy';
      case 'missingLocalFile':
        return 'Needs Local Copy';
      case 'localOnlyTrack':
        return 'Local Only';
      case 'inaccessibleCollection':
        return 'Inaccessible';
      case 'invalidLocalFile':
        return 'Invalid';
      case 'acquisitionFailed':
        return 'Failed';
    }
  }

  function issueChipClass(kind: IssueKind): string {
    if (
      kind === 'matchReview' ||
      kind === 'localOnlyTrack' ||
      kind === 'inaccessibleCollection'
    )
      return 'chip warning';
    if (kind === 'invalidLocalFile' || kind === 'acquisitionFailed')
      return 'chip error';
    return 'chip primary';
  }

  function score(candidate: MatchReviewCandidate): number {
    return Math.round(candidate.evidence.score.total / 100);
  }

  function isRejected(candidate: MatchReviewCandidate): boolean {
    return (
      review?.rejectedLibraryTrackIds.includes(candidate.track.id) ?? false
    );
  }

  function formatNumber(value: number): string {
    return new Intl.NumberFormat().format(value);
  }

  function evidenceLabels(candidate: MatchReviewCandidate): string[] {
    const labels: string[] = [];
    if (candidate.evidence.exactIsrc) labels.push('Exact ISRC');
    if (candidate.evidence.relationship === 'same')
      labels.push('Same recording');
    if (candidate.evidence.durationDifferenceMs !== null) {
      const seconds = Math.round(
        candidate.evidence.durationDifferenceMs / 1000,
      );
      labels.push(`Duration ${seconds >= 0 ? '+' : ''}${seconds}s`);
    }
    return labels;
  }
</script>

<div class="screen">
  <header class="page-header" style="padding-bottom:14px;">
    <div>
      <h1 class="page-title">Issues</h1>
      <p class="page-description">
        Resolve differences between your local library and tracked Spotify
        source state.
      </p>
    </div>
  </header>

  <div class="metrics-strip issues-summary">
    <div class="metric-inline attention">
      <div class="metric-value">{formatNumber(counts.localOnlyTrack)}</div>
      <div class="metric-label">local only</div>
    </div>
    <div class="metric-inline">
      <div class="metric-value">
        {formatNumber(counts.missingLocalFile + counts.matchReview)}
      </div>
      <div class="metric-label">Needs Local Copy</div>
    </div>
  </div>

  {#if error}<div class="error-state" style="margin-bottom:10px;">
      {error}
    </div>{/if}

  {#if loading && issues.length === 0}
    <div class="data-table" aria-label="Loading issues">
      {#each Array.from({ length: 8 }, (_, index) => index) as index (index)}
        <div class="skeleton" style="height:58px; margin-bottom:2px;"></div>
      {/each}
    </div>
  {:else if issues.length === 0}
    <div class="empty-state">
      <div>
        <strong>No unresolved issues</strong>
        <p>
          Refrain will show library or matching problems here when they need a
          decision.
        </p>
      </div>
    </div>
  {:else}
    <div class="issues-layout">
      <aside class="issue-queue">
        <div class="issue-queue-toolbar">
          <label class="search-field">
            <Icon name="search" size={16} />
            <input
              bind:value={search}
              class="search-input"
              aria-label="Search issues"
              placeholder="Search issues by title, artist, or album"
            />
          </label>
          <div class="issue-filter-row">
            <select
              bind:value={kindFilter}
              class="filter-select"
              aria-label="Issue type"
            >
              <option value="all">All Issues</option>
              <option value="localOnly">Local Only</option>
              <option value="trackedMissing">Needs Local Copy</option>
            </select>
            <div
              class="filter-select"
              style="display:flex; align-items:center; color:var(--text-secondary);"
            >
              {filteredIssues.length} issues
            </div>
          </div>
        </div>

        <div class="issue-list">
          {#each filteredIssues as issue (issue.id)}
            <button
              type="button"
              class:active={selectedIssue?.id === issue.id}
              class="issue-row"
              onclick={() => onSelect?.(issue)}
            >
              <div class="artwork-small artwork-fallback">
                {issue.title.slice(0, 1).toUpperCase()}
              </div>
              <div class="issue-row-copy">
                <p class="track-title">{issue.title}</p>
                <p class="track-secondary">
                  {issue.subtitle ?? issue.detail ?? 'Needs attention'}
                </p>
              </div>
              <span class={issueChipClass(issue.kind)}
                >{issueLabel(issue.kind)}</span
              >
              <Icon name="chevron-right" size={14} />
            </button>
          {/each}
        </div>
        {#if issues.length < total}
          <div style="padding:10px 0 0;">
            <button
              type="button"
              class="btn"
              style="width:100%;"
              onclick={() => onLoadMore?.()}
              disabled={loadingMore}
            >
              {loadingMore
                ? 'Loading…'
                : `Load More (${issues.length} of ${total})`}
            </button>
          </div>
        {/if}
      </aside>

      <section class="issue-detail">
        {#if selectedIssue}
          <div class="issue-detail-header">
            <div>
              <p style="margin:0; color:var(--text-secondary); font-size:10px;">
                {issueLabel(selectedIssue.kind)}
              </p>
              <h2
                style="margin:3px 0 0; font-size:20px; letter-spacing:-0.025em;"
              >
                {selectedIssue.kind === 'matchReview'
                  ? 'Ambiguous Match Review'
                  : selectedIssue.title}
              </h2>
              <p
                style="margin:3px 0 0; color:var(--text-secondary); font-size:10px;"
              >
                {selectedIssue.kind === 'matchReview'
                  ? 'Choose the best matching local track for this Spotify source item.'
                  : (selectedIssue.subtitle ??
                    'Review the current condition and resolve the underlying library state.')}
              </p>
            </div>
            <span class={issueChipClass(selectedIssue.kind)}
              >{issueLabel(selectedIssue.kind)}</span
            >
          </div>

          {#if selectedIssue.kind === 'matchReview'}
            {#if reviewLoading}
              <div
                class="skeleton"
                style="height:180px; margin-top:14px;"
              ></div>
            {:else if review}
              <section
                style="display:flex; align-items:center; gap:12px; padding:14px 2px 12px; border-bottom:1px solid var(--divider);"
              >
                <div
                  class="artwork-small artwork-fallback"
                  style="width:64px; height:64px; flex-basis:64px; font-size:18px;"
                >
                  {review.source.title.slice(0, 1).toUpperCase()}
                </div>
                <div style="min-width:0; flex:1;">
                  <h3 style="margin:0; font-size:14px;">
                    {review.source.title}
                  </h3>
                  <p
                    style="margin:3px 0 0; color:var(--text-secondary); font-size:10px;"
                  >
                    {review.source.artists.join(', ') || 'Unknown artist'}
                  </p>
                  <p
                    style="margin:3px 0 0; color:var(--text-tertiary); font-size:9px;"
                  >
                    {review.source.album ?? 'Unknown album'} · {formatTrackDuration(
                      review.source.durationMs,
                    )}{review.source.versionKind
                      ? ` · ${review.source.versionKind}`
                      : ''}
                  </p>
                </div>
              </section>

              <h3 style="margin:12px 0 0; font-size:12px;">
                Candidate Local Matches
              </h3>
              <div class="candidate-list">
                {#each review.candidates as candidate, index (candidate.track.id)}
                  <article
                    class:best={index === 0 && !isRejected(candidate)}
                    class:rejected={isRejected(candidate)}
                    class="candidate-card"
                  >
                    <div class="candidate-top">
                      <div style="display:flex; min-width:0; flex:1; gap:10px;">
                        <div
                          class="artwork-small artwork-fallback"
                          style="width:52px; height:52px; flex-basis:52px;"
                        >
                          {candidate.track.title.slice(0, 1).toUpperCase()}
                        </div>
                        <div style="min-width:0; flex:1;">
                          <p class="track-title" style="font-size:11px;">
                            {candidate.track.title}
                          </p>
                          <p class="track-secondary">
                            {candidate.track.artists.join(', ') ||
                              'Unknown artist'} · {candidate.track.album ??
                              'Unknown album'}
                          </p>
                          <p class="track-secondary">
                            {formatTrackDuration(
                              candidate.track.durationMs,
                            )}{candidate.track.isrc
                              ? ` · ISRC ${candidate.track.isrc}`
                              : ''}
                          </p>
                          <div
                            style="display:flex; flex-wrap:wrap; gap:5px; margin-top:7px;"
                          >
                            {#each evidenceLabels(candidate) as label (label)}<span
                                class="chip success">{label}</span
                              >{/each}
                            {#each candidate.evidence.warnings as warning (warning)}<span
                                class="chip warning">{warning}</span
                              >{/each}
                            {#each candidate.evidence.incompatibilities as warning (warning)}<span
                                class="chip error">{warning}</span
                              >{/each}
                          </div>
                        </div>
                      </div>
                      <div
                        style="display:flex; align-items:flex-start; gap:14px;"
                      >
                        <div class="candidate-score">
                          {score(candidate)}% <small>match</small>
                        </div>
                        <div class="candidate-actions">
                          {#if isRejected(candidate)}
                            <span
                              class="chip error"
                              style="justify-content:center;">Rejected</span
                            >
                            <button
                              type="button"
                              class="btn"
                              onclick={() =>
                                onClearRejection?.(
                                  review!.source.id,
                                  candidate.track.id,
                                )}
                              disabled={decisionBusy}>Clear Rejection</button
                            >
                          {:else}
                            <button
                              type="button"
                              class="btn btn-primary"
                              onclick={() =>
                                onConfirm?.(
                                  review!.source.id,
                                  candidate.track.id,
                                )}
                              disabled={decisionBusy}>Confirm Match</button
                            >
                            <button
                              type="button"
                              class="btn"
                              onclick={() =>
                                onReject?.(
                                  review!.source.id,
                                  candidate.track.id,
                                )}
                              disabled={decisionBusy}>Reject</button
                            >
                          {/if}
                        </div>
                      </div>
                    </div>

                    <div class="candidate-files">
                      <div
                        style="display:flex; align-items:center; gap:7px; color:var(--text-secondary); font-size:9px; font-weight:600;"
                      >
                        <Icon name="folder" size={13} /> Local Files ({candidate
                          .files.length})
                      </div>
                      {#if candidate.files.length === 0}
                        <p
                          style="margin:6px 0 0; color:var(--text-tertiary); font-size:9px;"
                        >
                          No local files linked to this candidate.
                        </p>
                      {:else}
                        {#each candidate.files as file (file.id)}
                          <div
                            style="display:flex; align-items:center; gap:7px; margin-top:6px; min-width:0; font-size:9px;"
                          >
                            <span
                              class={file.state === 'present'
                                ? 'chip success'
                                : file.state === 'invalid'
                                  ? 'chip error'
                                  : 'chip warning'}>{file.state}</span
                            >
                            <span
                              class="cell-truncate mono-path"
                              title={file.path}
                              style="flex:1; color:var(--text-secondary);"
                              >{file.path}</span
                            >
                            {#if file.isPreferred}<span class="chip"
                                >Preferred</span
                              >{/if}
                          </div>
                        {/each}
                      {/if}
                    </div>
                  </article>
                {/each}
              </div>
            {:else}
              <div class="empty-state" style="margin-top:14px;">
                Match details are no longer available. Refresh the issue list.
              </div>
            {/if}
          {:else}
            <div style="max-width:720px; padding-top:18px;">
              {#if selectedIssue.detail}<p
                  style="color:var(--text-secondary); font-size:11px; line-height:1.6;"
                >
                  {selectedIssue.detail}
                </p>{/if}
              {#if selectedIssue.path}
                <section class="inspector-card" style="margin-top:14px;">
                  <h3 class="inspector-title">File Path</h3>
                  <p
                    class="mono-path"
                    style="margin:0; overflow-wrap:anywhere; color:var(--text-secondary); font-size:10px; line-height:1.5;"
                  >
                    {selectedIssue.path}
                  </p>
                </section>
              {/if}
              <p
                style="color:var(--text-tertiary); font-size:10px; line-height:1.55;"
              >
                This issue disappears automatically after the underlying
                condition is resolved and Refrain refreshes its state.
              </p>
            </div>
          {/if}
        {:else}
          <div class="empty-state">No issues match the current filters.</div>
        {/if}
      </section>
    </div>
  {/if}
</div>
