<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
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
  export let trashBusyId: number | null = null;
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
  export let onTrashInvalid: ((localFileId: number) => void) | undefined =
    undefined;

  let search = '';
  let kindFilter: 'all' | 'match' | 'missing' | 'collection' | 'invalid' =
    'all';
  let sortOrder: 'priority' | 'title' | 'type' = 'priority';
  let filtersOpen = false;
  const numberFormatter = new Intl.NumberFormat();

  $: filteredIssues = issues
    .filter((issue) => {
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
      if (kindFilter === 'match') return issue.kind === 'matchReview';
      if (kindFilter === 'missing') return issue.kind === 'missingLocalFile';
      if (kindFilter === 'collection')
        return issue.kind === 'inaccessibleCollection';
      if (kindFilter === 'invalid') return issue.kind === 'invalidLocalFile';
      return true;
    })
    .sort(compareIssues);
  $: activeFilterCount =
    (kindFilter === 'all' ? 0 : 1) + (sortOrder === 'priority' ? 0 : 1);
  $: selectedIssue =
    filteredIssues.find((issue) => issue.id === selectedIssueId) ?? null;

  function issueLabel(kind: IssueKind): string {
    switch (kind) {
      case 'matchReview':
        return 'Match Review';
      case 'missingLocalFile':
        return 'Missing File';
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
      kind === 'missingLocalFile' ||
      kind === 'inaccessibleCollection'
    )
      return 'chip warning';
    if (kind === 'invalidLocalFile' || kind === 'acquisitionFailed')
      return 'chip error';
    return 'chip primary';
  }

  function issueStatusLabel(kind: IssueKind): string {
    if (kind === 'matchReview') return 'Needs Review';
    if (kind === 'missingLocalFile') return 'Missing';
    if (kind === 'invalidLocalFile') return 'Invalid';
    if (kind === 'inaccessibleCollection') return 'Inaccessible';
    return issueLabel(kind);
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
    return numberFormatter.format(value);
  }

  function issueIcon(kind: IssueKind): 'issues' | 'cloud' | 'link' | 'close' {
    if (kind === 'missingLocalFile') return 'cloud';
    if (kind === 'inaccessibleCollection') return 'link';
    if (kind === 'invalidLocalFile') return 'close';
    return 'issues';
  }

  function issuePriority(kind: IssueKind): number {
    if (kind === 'matchReview') return 0;
    if (kind === 'missingLocalFile') return 1;
    if (kind === 'invalidLocalFile') return 2;
    if (kind === 'inaccessibleCollection') return 3;
    return 4;
  }

  function compareIssues(left: IssueRow, right: IssueRow): number {
    if (sortOrder === 'title') return left.title.localeCompare(right.title);
    if (sortOrder === 'type') {
      return issueLabel(left.kind).localeCompare(issueLabel(right.kind));
    }
    return issuePriority(left.kind) - issuePriority(right.kind);
  }

  function clearIssueControls() {
    kindFilter = 'all';
    sortOrder = 'priority';
  }

  function localArtworkUrl(path: string | null): string | null {
    if (!path) return null;
    try {
      return convertFileSrc(path);
    } catch {
      return null;
    }
  }

  function issueArtworkUrl(issue: IssueRow): string | null {
    return issue.imageUrl || localArtworkUrl(issue.artworkPath);
  }

  function candidateArtworkUrl(candidate: MatchReviewCandidate): string | null {
    const artworkPath = candidate.files.find(
      (file) => file.artworkPath,
    )?.artworkPath;
    return localArtworkUrl(artworkPath ?? null) || candidate.track.imageUrl;
  }

  function candidateScoreTone(candidate: MatchReviewCandidate): string {
    const value = score(candidate);
    if (value >= 90) return 'strong';
    if (value >= 70) return 'medium';
    return 'weak';
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

<div class="screen issues-screen">
  <header class="page-header">
    <div>
      <h1 class="page-title">Issues</h1>
      <p class="page-description">
        Resolve library problems and ambiguous matches without leaving the
        queue.
      </p>
    </div>
  </header>

  <div class="issues-summary" aria-label="Issue summary">
    <strong>{formatNumber(total)} unresolved</strong>
    <span class:attention={counts.matchReview > 0}
      >{formatNumber(counts.matchReview)} review</span
    >
    <span>{formatNumber(counts.missingLocalFile)} missing</span>
    <span>{formatNumber(counts.inaccessibleCollection)} inaccessible</span>
    <span class:critical={counts.invalidLocalFile > 0}
      >{formatNumber(counts.invalidLocalFile)} invalid</span
    >
  </div>

  {#if error}
    <div class="error-state issues-error">{error}</div>
  {/if}

  {#if loading && issues.length === 0}
    <div class="issues-loading" aria-label="Loading issues">
      {#each Array.from({ length: 8 }, (_, index) => index) as index (index)}
        <div class="skeleton issue-row-skeleton"></div>
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
        <div class="toolbar issue-queue-toolbar">
          <label class="search-field">
            <Icon name="search" size={16} />
            <input
              bind:value={search}
              class="search-input"
              aria-label="Search issues"
              placeholder="Search issues by title, artist, or album"
            />
          </label>
          <button
            type="button"
            class="icon-button issue-filter-button"
            class:filters-active={activeFilterCount > 0}
            aria-expanded={filtersOpen}
            aria-label="Filter and sort issues"
            title="Filter and sort issues"
            onclick={() => (filtersOpen = !filtersOpen)}
          >
            <Icon name="filter" size={15} />
            {#if activeFilterCount > 0}
              <span class="issue-filter-count">{activeFilterCount}</span>
            {/if}
          </button>
        </div>

        {#if filtersOpen}
          <div class="filters-panel issues-filters-panel">
            <div class="filters-panel-header">
              <div class="filters-panel-title">Filter and sort issues</div>
              <button
                type="button"
                class="link-button"
                onclick={clearIssueControls}>Clear</button
              >
            </div>
            <div class="filters-grid">
              <label class="filter-field">
                <span class="filter-label">Issue type</span>
                <select
                  bind:value={kindFilter}
                  class="filter-select"
                  aria-label="Issue type"
                >
                  <option value="all">All issue types</option>
                  <option value="match">Needs review</option>
                  <option value="missing">Missing local file</option>
                  <option value="collection">Inaccessible source</option>
                  <option value="invalid">Invalid file</option>
                </select>
              </label>
              <label class="filter-field">
                <span class="filter-label">Sort by</span>
                <select
                  bind:value={sortOrder}
                  class="filter-select"
                  aria-label="Sort issues"
                >
                  <option value="priority">Priority</option>
                  <option value="title">Title</option>
                  <option value="type">Issue type</option>
                </select>
              </label>
            </div>
          </div>
        {/if}

        <div class="issue-queue-meta">
          <strong>{formatNumber(filteredIssues.length)} issues</strong>
          <span>{formatNumber(total)} total unresolved</span>
        </div>

        <div class="issue-list">
          {#each filteredIssues as issue (issue.id)}
            <button
              type="button"
              class:active={selectedIssue?.id === issue.id}
              class="issue-row"
              onclick={() => onSelect?.(issue)}
            >
              {#if issueArtworkUrl(issue)}
                <img
                  class="issue-row-artwork"
                  src={issueArtworkUrl(issue) ?? ''}
                  alt=""
                />
              {:else}
                <span class={`issue-kind-icon ${issue.kind}`}>
                  <Icon name={issueIcon(issue.kind)} size={15} />
                </span>
              {/if}
              <div class="issue-row-copy">
                <p class="issue-row-track">{issue.title}</p>
                <p class="issue-row-context">
                  {issue.subtitle ?? 'Needs attention'}
                </p>
              </div>
              <span class={issueChipClass(issue.kind)}
                >{issueStatusLabel(issue.kind)}</span
              >
            </button>
          {/each}
        </div>
        {#if issues.length < total}
          <div class="issue-load-more">
            <button
              type="button"
              class="btn"
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
            <div class="issue-detail-primary">
              {#if issueArtworkUrl(selectedIssue)}
                <img
                  class="issue-detail-artwork"
                  src={issueArtworkUrl(selectedIssue) ?? ''}
                  alt=""
                />
              {:else}
                <div
                  class={`issue-kind-icon issue-detail-artwork ${selectedIssue.kind}`}
                >
                  <Icon name={issueIcon(selectedIssue.kind)} size={18} />
                </div>
              {/if}
              <div class="issue-detail-heading">
                <p class="issue-detail-eyebrow">
                  {issueLabel(selectedIssue.kind)}
                </p>
                <h2 class="issue-detail-title">
                  {selectedIssue.kind === 'matchReview'
                    ? 'Ambiguous Match Review'
                    : selectedIssue.title}
                </h2>
                <p class="issue-detail-description">
                  {selectedIssue.kind === 'matchReview'
                    ? 'Choose the best matching local track for this Spotify source item.'
                    : (selectedIssue.subtitle ??
                      'Review the current condition and resolve the underlying library state.')}
                </p>
              </div>
            </div>
            <span class={issueChipClass(selectedIssue.kind)}
              >{issueLabel(selectedIssue.kind)}</span
            >
          </div>

          {#if selectedIssue.kind === 'matchReview'}
            {#if reviewLoading}
              <div class="skeleton issue-detail-skeleton"></div>
            {:else if review}
              <section class="issue-source">
                <p class="issue-source-label">Spotify source</p>
                <div class="issue-source-main">
                  <div class="issue-source-track">
                    {#if review.source.imageUrl}
                      <img
                        class="artwork-small issue-source-artwork"
                        src={review.source.imageUrl}
                        alt=""
                      />
                    {:else}
                      <div
                        class="artwork-small artwork-fallback issue-source-artwork"
                      >
                        {review.source.title.slice(0, 1).toUpperCase()}
                      </div>
                    {/if}
                    <div class="issue-source-copy">
                      <h3>
                        {review.source.title}
                      </h3>
                      <p class="issue-source-artist">
                        {review.source.artists.join(', ') || 'Unknown artist'}
                      </p>
                      <p class="issue-source-meta">
                        {review.source.album ?? 'Unknown album'} · {formatTrackDuration(
                          review.source.durationMs,
                        )}{review.source.versionKind
                          ? ` · ${review.source.versionKind}`
                          : ''}
                      </p>
                    </div>
                  </div>
                  <div class="issue-source-status">
                    <span class="chip warning">
                      <Icon name="warning" size={12} /> Needs Review
                    </span>
                    <span class="issue-source-isrc"
                      >ISRC {review.source.isrc ?? '—'}</span
                    >
                  </div>
                </div>
              </section>

              <div class="candidate-section-heading">
                <div>
                  <h3>Candidate local matches</h3>
                  <p>
                    Compare evidence and choose the recording that belongs to
                    the Spotify source.
                  </p>
                </div>
                <span>{review.candidates.length} candidates</span>
              </div>

              <div class="candidate-list">
                {#each review.candidates as candidate, index (candidate.track.id)}
                  <article
                    class:best={index === 0 && !isRejected(candidate)}
                    class:rejected={isRejected(candidate)}
                    class="candidate-card"
                  >
                    <div class="candidate-header">
                      <div class="candidate-main">
                        {#if candidateArtworkUrl(candidate)}
                          <img
                            class="artwork-small candidate-artwork"
                            src={candidateArtworkUrl(candidate) ?? ''}
                            alt=""
                          />
                        {:else}
                          <div
                            class="artwork-small artwork-fallback candidate-artwork"
                          >
                            {candidate.track.title.slice(0, 1).toUpperCase()}
                          </div>
                        {/if}
                        <div class="candidate-copy">
                          <p class="track-title">{candidate.track.title}</p>
                          <p class="track-secondary">
                            {candidate.track.artists.join(', ') ||
                              'Unknown artist'}
                          </p>
                          <p class="track-secondary">
                            {candidate.track.album ?? 'Unknown album'} · {formatTrackDuration(
                              candidate.track.durationMs,
                            )}{candidate.files[0]?.format
                              ? ` · ${candidate.files[0].format}`
                              : ''}
                          </p>
                          <p class="candidate-isrc">
                            ISRC {candidate.track.isrc ?? '—'}
                          </p>
                        </div>
                      </div>
                      <div
                        class={`candidate-score ${candidateScoreTone(candidate)}`}
                        aria-label={`${score(candidate)} percent match`}
                      >
                        <span
                          class="candidate-score-ring"
                          style={`--candidate-score:${score(candidate)}%`}
                        ></span>
                        <div>
                          <strong>{score(candidate)}%</strong>
                          <small>match</small>
                        </div>
                      </div>
                    </div>

                    <div class="candidate-evidence">
                      {#each evidenceLabels(candidate) as label (label)}<span
                          class="chip">{label}</span
                        >{/each}
                      {#each candidate.evidence.warnings as warning (warning)}<span
                          class="chip warning">{warning}</span
                        >{/each}
                      {#each candidate.evidence.incompatibilities as warning (warning)}<span
                          class="chip error">{warning}</span
                        >{/each}
                    </div>

                    <div class="candidate-files">
                      <div class="candidate-files-heading">
                        <Icon name="folder" size={13} /> Local Files ({candidate
                          .files.length})
                      </div>
                      {#if candidate.files.length === 0}
                        <p class="candidate-files-empty">
                          No local files linked to this candidate.
                        </p>
                      {:else}
                        {#each candidate.files as file (file.id)}
                          <div class="candidate-file-row">
                            <span
                              class={file.state === 'present'
                                ? 'chip success'
                                : file.state === 'invalid'
                                  ? 'chip error'
                                  : 'chip warning'}>{file.state}</span
                            >
                            <span
                              class="cell-truncate mono-path"
                              title={file.path}>{file.path}</span
                            >
                            {#if file.isPreferred}<span class="chip"
                                >Preferred</span
                              >{/if}
                          </div>
                        {/each}
                      {/if}
                    </div>

                    <footer class="candidate-footer">
                      <span class="candidate-rank">
                        {index === 0 && !isRejected(candidate)
                          ? 'Best candidate'
                          : 'Local candidate'}
                      </span>
                      <div class="candidate-actions">
                        {#if isRejected(candidate)}
                          <span class="chip error candidate-rejected"
                            >Rejected</span
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
                            class="btn"
                            onclick={() =>
                              onReject?.(review!.source.id, candidate.track.id)}
                            disabled={decisionBusy}>Reject</button
                          >
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
                        {/if}
                      </div>
                    </footer>
                  </article>
                {/each}
              </div>
            {:else}
              <div class="empty-state issue-detail-empty">
                Match details are no longer available. Refresh the issue list.
              </div>
            {/if}
          {:else}
            <div class="issue-detail-body">
              {#if selectedIssue.detail}
                <p class="issue-detail-message">
                  {selectedIssue.detail}
                </p>
              {/if}
              {#if selectedIssue.path}
                <section class="inspector-card issue-path-card">
                  <h3 class="inspector-title">File Path</h3>
                  <p class="mono-path issue-path">
                    {selectedIssue.path}
                  </p>
                </section>
              {/if}
              {#if selectedIssue.kind === 'invalidLocalFile' && selectedIssue.localFileId !== null}
                <div class="issue-resolution-actions">
                  <button
                    type="button"
                    class="btn issue-trash-button"
                    onclick={() =>
                      onTrashInvalid?.(selectedIssue!.localFileId!)}
                    disabled={trashBusyId !== null}
                  >
                    <Icon name="trash" size={13} />
                    {trashBusyId === selectedIssue.localFileId
                      ? 'Moving to Trash…'
                      : 'Move to Trash'}
                  </button>
                  <span>
                    Removes this invalid file from the library folder using the
                    system Trash or Recycle Bin.
                  </span>
                </div>
              {/if}
              <p class="issue-resolution-note">
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
