<script lang="ts">
  import Icon from './Icon.svelte';
  import type { AcquisitionCandidate } from '../lib/acquisition';
  import type { StagingItem, StagingStage } from '../lib/staging';
  import type { SyncRun } from '../lib/sync';

  export let items: StagingItem[] = [];
  export let total = 0;
  export let selectedLibraryTrackId: number | null = null;
  export let selectedTrackIds: number[] = [];
  export let loading = false;
  export let busy = false;
  export let error: string | null = null;
  export let syncRun: SyncRun | null = null;
  export let syncBusy = false;
  export let syncScope: 'local' | 'spotify' | null = null;
  export let onSelect: ((item: StagingItem) => void) | undefined = undefined;
  export let onSelectionChange: ((trackIds: number[]) => void) | undefined =
    undefined;
  export let onStartSelected: (() => void) | undefined = undefined;
  export let onCancelSelected: (() => void) | undefined = undefined;
  export let onProcessSelected: (() => void) | undefined = undefined;
  export let onExclude: ((libraryTrackIds: number[]) => void) | undefined =
    undefined;
  export let onResolve:
    | ((jobId: number, provider: string, providerToken: string) => void)
    | undefined = undefined;
  export let onReject:
    | ((jobId: number, provider: string, providerToken: string) => void)
    | undefined = undefined;
  export let onSearchAgain:
    ((jobId: number) => void | Promise<void>) | undefined = undefined;
  export let onCancelSync: (() => void) | undefined = undefined;

  let search = '';
  let stageFilter: 'all' | StagingStage = 'all';
  let retryingJobId: number | null = null;

  $: filteredItems = items.filter((item) => {
    const query = search.trim().toLocaleLowerCase();
    if (
      query &&
      ![item.title, item.artists.join(', '), item.album ?? ''].some((value) =>
        value.toLocaleLowerCase().includes(query),
      )
    )
      return false;
    return stageFilter === 'all' || item.stage === stageFilter;
  });
  $: selected =
    items.find((item) => item.libraryTrackId === selectedLibraryTrackId) ??
    filteredItems[0] ??
    null;
  $: resolutionProviderGroups = selected
    ? groupResolutionCandidates(selected)
    : [];
  $: selectedIdSet = new Set(selectedTrackIds);
  $: selectedItems = items.filter((item) =>
    selectedIdSet.has(item.libraryTrackId),
  );
  $: selectedStartable = selectedItems.filter((item) =>
    ['needsLocalCopy', 'needsResolution', 'failed', 'cancelled'].includes(
      item.stage,
    ),
  );
  $: selectedActive = selectedItems.filter((item) =>
    ['queued', 'searching', 'downloading'].includes(item.stage),
  );
  $: selectedDownloaded = selectedItems.filter(
    (item) => item.stage === 'downloaded',
  );
  $: selectedExcludable = selectedItems.filter(
    (item) => !['queued', 'searching', 'downloading'].includes(item.stage),
  );
  $: allFilteredSelected =
    filteredItems.length > 0 &&
    filteredItems.every((item) => selectedIdSet.has(item.libraryTrackId));
  $: activeTransfer =
    items.find(
      (item) =>
        item.libraryTrackId === selectedLibraryTrackId &&
        item.stage === 'downloading',
    ) ??
    items.find((item) => item.stage === 'downloading') ??
    null;
  $: needsLocalCopy = items.filter(
    (item) => item.stage === 'needsLocalCopy' || item.stage === 'cancelled',
  ).length;
  $: active = items.filter((item) =>
    ['queued', 'searching', 'downloading'].includes(item.stage),
  ).length;
  $: needsResolution = items.filter(
    (item) => item.stage === 'needsResolution',
  ).length;
  $: failed = items.filter((item) => item.stage === 'failed').length;

  const spotifySyncSteps = [
    ['refreshSource', 'Refresh Spotify'],
    ['scanLocalLibrary', 'Scan local library'],
    ['resolveExistingLinks', 'Resolve existing matches'],
    ['matchUnresolved', 'Match unresolved tracks'],
    ['createMissing', 'Identify missing tracks'],
    ['acquireMissing', 'Download missing audio'],
    ['verifyImports', 'Verify and import'],
    ['normalizeFiles', 'Normalize library'],
  ] as const;
  const localSyncSteps = [
    ['scanLocalLibrary', 'Scan local library'],
    ['compareWithSpotify', 'Compare with Spotify'],
    ['normalizeFiles', 'Normalize library'],
  ] as const;

  function stageLabel(stage: StagingStage): string {
    switch (stage) {
      case 'needsLocalCopy':
        return 'Needs Local Copy';
      case 'queued':
        return 'Queued';
      case 'searching':
        return 'Searching';
      case 'downloading':
        return 'Downloading';
      case 'needsResolution':
        return 'Needs Resolution';
      case 'failed':
        return 'Failed';
      case 'downloaded':
        return 'Downloaded';
      case 'cancelled':
        return 'Cancelled';
    }
  }

  function chipClass(stage: StagingStage): string {
    if (stage === 'failed') return 'chip error';
    if (stage === 'needsResolution' || stage === 'needsLocalCopy')
      return 'chip warning';
    if (stage === 'downloaded') return 'chip success';
    if (stage === 'cancelled') return 'chip';
    return 'chip primary';
  }

  function syncChipClass(run: SyncRun | null): string {
    if (!run || run.status === 'running') return 'chip primary';
    if (run.status === 'succeeded') return 'chip success';
    if (run.status === 'partial') return 'chip warning';
    if (run.status === 'failed') return 'chip error';
    return 'chip';
  }

  function syncStatusLabel(run: SyncRun | null): string {
    if (!run) return syncBusy ? 'Starting' : 'Idle';
    if (run.status === 'running') return 'Running';
    if (run.status === 'succeeded') return 'Complete';
    if (run.status === 'partial') return 'Needs Attention';
    if (run.status === 'failed') return 'Failed';
    return 'Cancelled';
  }

  function phaseLabel(phase: string | null): string {
    const allSteps = [...spotifySyncSteps, ...localSyncSteps];
    return (
      allSteps.find(([value]) => value === phase)?.[1] ??
      'Preparing synchronization'
    );
  }

  function activity(item: StagingItem): string {
    switch (item.stage) {
      case 'needsLocalCopy':
        return 'Tracked and waiting for a local copy.';
      case 'queued':
        return 'Waiting for acquisition to start.';
      case 'searching':
        return 'Searching providers for a match.';
      case 'downloading':
        return item.candidate?.format
          ? `Downloading ${item.candidate.format.toUpperCase()} audio.`
          : 'Downloading the selected lossless audio.';
      case 'needsResolution':
        return `Review ${Math.max(item.candidates.length, 1)} possible ${item.candidates.length === 1 ? 'match' : 'matches'}.`;
      case 'failed':
        return item.errorMessage ?? 'Acquisition failed.';
      case 'downloaded':
        return 'Downloaded. Waiting for verification and import retry.';
      case 'cancelled':
        return 'Acquisition was cancelled.';
    }
  }

  function progressPercent(item: StagingItem): number | null {
    if (
      item.bytesTransferred === null ||
      item.totalBytes === null ||
      item.totalBytes <= 0
    )
      return null;
    return Math.max(
      0,
      Math.min(
        100,
        Math.round((item.bytesTransferred / item.totalBytes) * 100),
      ),
    );
  }

  async function retryFailedItem(item: StagingItem) {
    if (item.jobId === null || busy || retryingJobId !== null) return;
    retryingJobId = item.jobId;
    try {
      await onSearchAgain?.(item.jobId);
    } finally {
      retryingJobId = null;
    }
  }

  function formatBytes(value: number | null): string {
    if (value === null || value < 0) return '—';
    const units = ['B', 'KB', 'MB', 'GB'];
    let amount = value;
    let unit = 0;
    while (amount >= 1024 && unit < units.length - 1) {
      amount /= 1024;
      unit += 1;
    }
    return `${amount >= 10 || unit === 0 ? amount.toFixed(0) : amount.toFixed(1)} ${units[unit]}`;
  }

  function formatDuration(value: number | null): string {
    if (value === null || value < 0) return '—';
    const seconds = Math.round(value / 1000);
    const minutes = Math.floor(seconds / 60);
    return `${minutes}:${String(seconds % 60).padStart(2, '0')}`;
  }

  function formatRunTime(value: number | null): string {
    if (value === null) return '—';
    return new Intl.DateTimeFormat(undefined, {
      hour: 'numeric',
      minute: '2-digit',
      second: '2-digit',
    }).format(new Date(value));
  }

  function candidateIdentitySummary(candidate: AcquisitionCandidate): string {
    return [
      candidate.artists.join(', '),
      candidate.album,
      candidate.durationMs === null
        ? null
        : formatDuration(candidate.durationMs),
    ]
      .filter(Boolean)
      .join(' · ');
  }

  function candidateFileSummary(candidate: AcquisitionCandidate): string {
    return [
      candidate.format?.toUpperCase(),
      formatSampleRate(candidate.sampleRateHz),
      candidate.bitDepth && candidate.bitDepth > 0
        ? `${candidate.bitDepth}-bit`
        : null,
      candidate.bitrateKbps && candidate.bitrateKbps > 0
        ? `${candidate.bitrateKbps} kbps`
        : null,
      candidate.sizeBytes === null ? null : formatBytes(candidate.sizeBytes),
    ]
      .filter(Boolean)
      .join(' · ');
  }

  function candidateQualitySummary(candidate: AcquisitionCandidate): string {
    return [
      formatSampleRate(candidate.sampleRateHz),
      candidate.bitDepth && candidate.bitDepth > 0
        ? `${candidate.bitDepth}-bit`
        : null,
    ]
      .filter(Boolean)
      .join(' · ');
  }

  function itemQualitySummary(item: StagingItem): string {
    if (item.candidate) return candidateQualitySummary(item.candidate) || '—';
    if (item.stage !== 'needsResolution' || item.candidates.length === 0)
      return '—';

    const known = item.candidates
      .map(candidateQualitySummary)
      .filter((quality) => quality.length > 0);
    if (known.length === 0) return '—';
    const unique = [...new Set(known)];
    if (known.length !== item.candidates.length) return 'See matches';
    return unique.length === 1 ? unique[0] : 'Varies';
  }

  function formatSampleRate(value: number | null | undefined): string | null {
    if (!value || value <= 0) return null;
    const kilohertz = value / 1000;
    return `${Number.isInteger(kilohertz) ? kilohertz.toFixed(0) : kilohertz.toFixed(1)} kHz`;
  }

  function candidateIdentifiers(candidate: AcquisitionCandidate): string {
    return [
      candidate.isrc ? `ISRC ${candidate.isrc}` : null,
      candidate.releaseId ? `Release ${candidate.releaseId}` : null,
      candidate.recordingId ? `Recording ${candidate.recordingId}` : null,
    ]
      .filter(Boolean)
      .join(' · ');
  }

  function confidenceLabel(candidate: AcquisitionCandidate): string {
    return candidate.confidence === null || candidate.confidence === undefined
      ? 'Unscored'
      : `${candidate.confidence}%`;
  }

  function candidateProvider(
    item: StagingItem,
    candidate: AcquisitionCandidate,
  ): string {
    return candidate.provider ?? item.provider ?? 'unknown';
  }

  function providerLabel(provider: string): string {
    if (provider === 'monochrome') return 'Monochrome';
    if (provider === 'antra') return 'Antra';
    if (provider === 'sockseek') return 'Sockseek';
    return provider === 'unknown' ? 'Provider' : provider;
  }

  function providerSummary(item: StagingItem): string {
    if (item.stage === 'needsResolution') {
      const providers = new Set(
        item.candidates
          .map((candidate) => candidateProvider(item, candidate))
          .filter((provider) => provider !== 'unknown'),
      );
      if (providers.size > 1) return `${providers.size} providers`;
      const provider = providers.values().next().value as string | undefined;
      if (provider) return providerLabel(provider);
    }
    return item.provider ? providerLabel(item.provider) : 'Not started';
  }

  function candidateFormatPriority(candidate: AcquisitionCandidate): number {
    const format = candidate.format?.trim().toLocaleLowerCase() ?? '';
    if (format === 'flac') return 3;
    if (['alac', 'wav', 'wave', 'aiff', 'aif', 'ape', 'wv'].includes(format))
      return 2;
    return format ? 1 : 0;
  }

  function candidateQualityCompare(
    left: AcquisitionCandidate,
    right: AcquisitionCandidate,
  ): number {
    return (
      candidateFormatPriority(right) - candidateFormatPriority(left) ||
      (right.bitDepth ?? 0) - (left.bitDepth ?? 0) ||
      (right.sampleRateHz ?? 0) - (left.sampleRateHz ?? 0) ||
      (right.bitrateKbps ?? 0) - (left.bitrateKbps ?? 0)
    );
  }

  function groupResolutionCandidates(item: StagingItem) {
    const groups: Array<{
      provider: string;
      candidates: AcquisitionCandidate[];
    }> = [];
    for (const candidate of item.candidates) {
      const provider = candidateProvider(item, candidate);
      let group = groups.find((entry) => entry.provider === provider);
      if (!group) {
        group = { provider, candidates: [] };
        groups.push(group);
      }
      group.candidates.push(candidate);
    }
    for (const group of groups) {
      group.candidates.sort(
        (left, right) =>
          (right.confidence ?? 0) - (left.confidence ?? 0) ||
          candidateQualityCompare(left, right),
      );
    }
    return groups;
  }

  function toggleTrackSelection(item: StagingItem, checked: boolean) {
    const next = new Set(selectedTrackIds);
    if (checked) next.add(item.libraryTrackId);
    else next.delete(item.libraryTrackId);
    onSelectionChange?.([...next]);
  }

  function toggleFilteredSelection(checked: boolean) {
    const next = new Set(selectedTrackIds);
    for (const item of filteredItems) {
      if (checked) next.add(item.libraryTrackId);
      else next.delete(item.libraryTrackId);
    }
    onSelectionChange?.([...next]);
  }

  function syncSteps(run: SyncRun | null) {
    if (run?.scope === 'local') return localSyncSteps;
    return spotifySyncSteps;
  }

  function syncStepState(
    run: SyncRun | null,
    phase: string,
  ): 'done' | 'current' | 'pending' {
    if (!run) return 'pending';
    const steps = syncSteps(run);
    const currentIndex = steps.findIndex(([value]) => value === run.phase);
    const stepIndex = steps.findIndex(([value]) => value === phase);
    if (run.status === 'succeeded' || run.status === 'partial') return 'done';
    if (stepIndex < currentIndex) return 'done';
    if (stepIndex === currentIndex) return 'current';
    return 'pending';
  }
</script>

<div class="screen staging-screen">
  <header class="page-header staging-header">
    <div>
      <h1 class="page-title">Staging</h1>
      <p class="page-description">
        Downloads, verification, import, and live synchronization detail.
      </p>
    </div>
    <div class="staging-header-status" aria-label="Staging summary">
      {#if active > 0}<span><strong>{active}</strong> active</span>{/if}
      {#if needsResolution > 0}<span
          ><strong>{needsResolution}</strong> review</span
        >{/if}
      {#if failed > 0}<span><strong>{failed}</strong> failed</span>{/if}
      {#if needsLocalCopy > 0}<span
          ><strong>{needsLocalCopy}</strong> waiting</span
        >{/if}
    </div>
  </header>

  {#if error}<div class="error-state staging-error">{error}</div>{/if}

  <div class="staging-shell">
    {#if loading && items.length === 0 && !syncRun}
      <div class="staging-loading" aria-label="Loading staging">
        <div class="skeleton" style="height:68px;"></div>
        <div class="skeleton" style="height:68px;"></div>
        <div class="skeleton" style="height:68px;"></div>
      </div>
    {:else if items.length === 0}
      <div class="sync-dashboard">
        <section class="sync-primary-card">
          <div class="sync-card-header">
            <div>
              <p class="eyebrow">Synchronization</p>
              <h2>
                {syncRun
                  ? phaseLabel(syncRun.phase)
                  : syncBusy
                    ? 'Preparing synchronization'
                    : 'Library is clear'}
              </h2>
              <p>
                {#if syncRun?.status === 'running'}
                  Refrain is working through the current sync automatically.
                {:else if syncRun?.status === 'partial'}
                  The run completed, but some tracks still need attention.
                {:else if syncRun?.status === 'failed'}
                  {syncRun.errorMessage ??
                    'The last synchronization did not complete.'}
                {:else if syncRun?.status === 'cancelled'}
                  The last synchronization was cancelled.
                {:else if syncRun}
                  The last synchronization completed and nothing is waiting to
                  download.
                {:else}
                  Start Sync Library to scan, reconcile, download, verify, and
                  import tracked music.
                {/if}
              </p>
            </div>
            <span class={syncChipClass(syncRun)}
              >{syncStatusLabel(syncRun)}</span
            >
          </div>

          <div class="sync-timeline" aria-label="Synchronization phases">
            {#each syncSteps(syncRun) as [phase, label] (phase)}
              {@const state = syncStepState(syncRun, phase)}
              <div
                class:done={state === 'done'}
                class:current={state === 'current'}
                class="sync-step"
              >
                <span class="sync-step-dot"></span>
                <div>
                  <strong>{label}</strong>
                  <small
                    >{state === 'done'
                      ? 'Complete'
                      : state === 'current'
                        ? 'In progress'
                        : 'Waiting'}</small
                  >
                </div>
              </div>
            {/each}
          </div>
        </section>

        <aside class="sync-summary-card">
          <div class="sync-summary-title">
            <div>
              <p class="eyebrow">Latest run</p>
              <h3>
                {syncRun?.scope === 'local'
                  ? 'Local reconciliation'
                  : 'Spotify sync'}
              </h3>
            </div>
            {#if syncBusy}
              <button type="button" class="btn" onclick={() => onCancelSync?.()}
                >Cancel</button
              >
            {/if}
          </div>

          <dl class="sync-run-facts">
            <div>
              <dt>Started</dt>
              <dd>{formatRunTime(syncRun?.startedAt ?? null)}</dd>
            </div>
            <div>
              <dt>Finished</dt>
              <dd>{formatRunTime(syncRun?.finishedAt ?? null)}</dd>
            </div>
            <div>
              <dt>Matched</dt>
              <dd>{syncRun?.matched ?? 0}</dd>
            </div>
            <div>
              <dt>Missing</dt>
              <dd>{syncRun?.missing ?? 0}</dd>
            </div>
            <div>
              <dt>Needs review</dt>
              <dd>{syncRun?.needsReview ?? 0}</dd>
            </div>
            <div>
              <dt>Failed acquisition</dt>
              <dd>{syncRun?.acquisitionFailed ?? 0}</dd>
            </div>
          </dl>
        </aside>
      </div>
    {:else}
      <div class="staging-main">
        <section class="queue-panel">
          <div class="queue-toolbar">
            <label class="select-all" title="Select all visible tracks">
              <input
                type="checkbox"
                aria-label="Select all visible staging tracks"
                checked={allFilteredSelected}
                onchange={(event) =>
                  toggleFilteredSelection(event.currentTarget.checked)}
              />
            </label>
            <label class="search-field">
              <Icon name="search" size={15} />
              <input
                bind:value={search}
                class="search-input"
                aria-label="Search staging"
                placeholder="Search downloads"
              />
            </label>
            <select
              bind:value={stageFilter}
              class="filter-select queue-filter"
              aria-label="Staging state"
            >
              <option value="all">All states</option>
              <option value="needsLocalCopy">Needs local copy</option>
              <option value="queued">Queued</option>
              <option value="searching">Searching</option>
              <option value="downloading">Downloading</option>
              <option value="needsResolution">Needs resolution</option>
              <option value="failed">Failed</option>
              <option value="downloaded">Downloaded</option>
              <option value="cancelled">Cancelled</option>
            </select>
            <span class="queue-count">{filteredItems.length} of {total}</span>
          </div>

          {#if selectedItems.length > 0}
            <div
              class="selection-toolbar"
              aria-label="Selected staging actions"
            >
              <strong>{selectedItems.length} selected</strong>
              <div class="selection-actions">
                {#if selectedStartable.length > 0}
                  <button
                    type="button"
                    class="btn btn-primary"
                    disabled={busy}
                    onclick={() => onStartSelected?.()}
                  >
                    {selectedStartable.every((item) =>
                      ['needsResolution', 'failed', 'cancelled'].includes(
                        item.stage,
                      ),
                    )
                      ? 'Retry'
                      : 'Start'}
                    {selectedStartable.length}
                  </button>
                {/if}
                {#if selectedActive.length > 0}
                  <button
                    type="button"
                    class="btn"
                    disabled={busy}
                    onclick={() => onCancelSelected?.()}
                  >
                    Cancel {selectedActive.length}
                  </button>
                {/if}
                {#if selectedDownloaded.length > 0}
                  <button
                    type="button"
                    class="btn"
                    disabled={busy}
                    onclick={() => onProcessSelected?.()}
                  >
                    Process {selectedDownloaded.length}
                  </button>
                {/if}
                {#if selectedExcludable.length > 0}
                  <button
                    type="button"
                    class="btn staging-exclude-selected"
                    disabled={busy}
                    onclick={() =>
                      onExclude?.(
                        selectedExcludable.map((item) => item.libraryTrackId),
                      )}
                  >
                    <Icon name="close" size={12} strokeWidth={2} />
                    Exclude from tracking
                  </button>
                {/if}
                <button
                  type="button"
                  class="text-button"
                  onclick={() => onSelectionChange?.([])}>Clear</button
                >
              </div>
            </div>
          {/if}

          <div class="download-list">
            {#each filteredItems as item (item.libraryTrackId)}
              {@const percent = progressPercent(item)}
              <article
                class:selected-card={selected?.libraryTrackId ===
                  item.libraryTrackId}
                class="download-card"
              >
                <label
                  class="card-checkbox"
                  aria-label={`Select ${item.title}`}
                >
                  <input
                    type="checkbox"
                    checked={selectedIdSet.has(item.libraryTrackId)}
                    onchange={(event) =>
                      toggleTrackSelection(item, event.currentTarget.checked)}
                  />
                </label>
                <div class="download-card-content">
                  <button
                    type="button"
                    class="download-card-main"
                    onclick={() => onSelect?.(item)}
                  >
                    <div class="download-artwork artwork-fallback">
                      {#if item.imageUrl}
                        <img
                          src={item.imageUrl}
                          alt={`${item.title} artwork`}
                        />
                      {:else}
                        <span>{item.title.slice(0, 1).toUpperCase()}</span>
                      {/if}
                    </div>
                    <div class="download-copy">
                      <div class="download-title-row">
                        <p class="track-title">{item.title}</p>
                        <span class={chipClass(item.stage)}
                          >{stageLabel(item.stage)}</span
                        >
                      </div>
                      <p class="track-secondary">
                        {item.artists.join(', ') || 'Unknown artist'}
                      </p>
                      <p class="download-album">
                        {item.album ?? 'Unknown album'} · {formatDuration(
                          item.durationMs,
                        )}
                      </p>
                    </div>
                    <div class="download-meta">
                      {#if percent !== null}<strong>{percent}%</strong>{/if}
                      {#if item.candidate?.format}
                        <small>{item.candidate.format.toUpperCase()}</small>
                        {#if candidateQualitySummary(item.candidate)}
                          <small class="download-quality"
                            >{candidateQualitySummary(item.candidate)}</small
                          >
                        {/if}
                        <small class="download-provider"
                          >{providerSummary(item)}</small
                        >
                      {:else}
                        <small>{providerSummary(item)}</small>
                      {/if}
                    </div>
                  </button>
                  <p class="download-activity">{activity(item)}</p>
                  {#if item.stage === 'downloading'}
                    <div
                      class:indeterminate={percent === null}
                      class="card-progress"
                    >
                      <span style={percent === null ? '' : `width:${percent}%`}
                      ></span>
                    </div>
                  {/if}
                </div>
              </article>
            {/each}
          </div>
        </section>

        <aside
          class="track-inspector"
          class:resolution-inspector={selected?.stage === 'needsResolution'}
        >
          {#if selected}
            <div class="inspector-summary">
              <div class="inspector-artwork artwork-fallback">
                {#if selected.imageUrl}
                  <img
                    src={selected.imageUrl}
                    alt={`${selected.title} artwork`}
                  />
                {:else}
                  <span>{selected.title.slice(0, 1).toUpperCase()}</span>
                {/if}
              </div>
              <div class="inspector-heading">
                <span class={chipClass(selected.stage)}
                  >{stageLabel(selected.stage)}</span
                >
                <h2>{selected.title}</h2>
                <p>{selected.artists.join(', ') || 'Unknown artist'}</p>
              </div>
            </div>

            <dl class="inspector-facts">
              <div>
                <dt>Album</dt>
                <dd>{selected.album ?? 'Unknown album'}</dd>
              </div>
              <div>
                <dt>Duration</dt>
                <dd>{formatDuration(selected.durationMs)}</dd>
              </div>
              <div>
                <dt>Format</dt>
                <dd>{selected.candidate?.format?.toUpperCase() ?? '—'}</dd>
              </div>
              <div>
                <dt>Quality</dt>
                <dd>{itemQualitySummary(selected)}</dd>
              </div>
              <div>
                <dt>Provider</dt>
                <dd>{providerSummary(selected)}</dd>
              </div>
              <div>
                <dt>Attempt</dt>
                <dd>{selected.attempt || '—'}</dd>
              </div>
              <div>
                <dt>Size</dt>
                <dd>{formatBytes(selected.candidate?.sizeBytes ?? null)}</dd>
              </div>
            </dl>

            {#if selected.stagingPath}
              <div class="inspector-path">
                <span>Staging path</span>
                <code title={selected.stagingPath}>{selected.stagingPath}</code>
              </div>
            {/if}

            {#if selected.errorMessage && selected.stage !== 'needsResolution'}
              <div class="inspector-error">
                <strong>{selected.errorCode ?? 'Download error'}</strong>
                <p>{selected.errorMessage}</p>
                {#if selected.stage === 'failed' && selected.jobId !== null}
                  <button
                    type="button"
                    class="btn inspector-retry"
                    disabled={busy || retryingJobId === selected.jobId}
                    onclick={() => void retryFailedItem(selected)}
                  >
                    <Icon name="refresh" size={12} strokeWidth={2} />
                    {retryingJobId === selected.jobId ? 'Retrying…' : 'Retry'}
                  </button>
                {/if}
              </div>
            {/if}

            {#if selected.stage === 'needsResolution' && selected.jobId !== null}
              <div class="resolution-section">
                <div class="resolution-heading">
                  <div>
                    <h3>Choose a match</h3>
                    <p>
                      Confirm the recording Refrain should download and add.
                    </p>
                  </div>
                  <button
                    type="button"
                    class="text-button resolution-search"
                    onclick={() => onSearchAgain?.(selected.jobId!)}
                  >
                    <Icon name="refresh" size={12} strokeWidth={2} />
                    <span>Search again</span>
                  </button>
                </div>
                <div class="candidate-stack">
                  {#each resolutionProviderGroups as group (group.provider)}
                    <section class="candidate-provider-group">
                      <div class="candidate-provider-heading">
                        <strong>{providerLabel(group.provider)}</strong>
                        <span
                          >{group.candidates.length} result{group.candidates
                            .length === 1
                            ? ''
                            : 's'}</span
                        >
                      </div>
                      {#each group.candidates as candidate (`${group.provider}:${candidate.providerToken}`)}
                        <article class="candidate-card staging-candidate-card">
                          <div class="candidate-card-heading">
                            <p class="track-title">
                              {candidate.title ?? selected.title}
                            </p>
                            <span
                              class="candidate-confidence"
                              title="Match confidence"
                              >{confidenceLabel(candidate)}</span
                            >
                          </div>
                          <p class="track-secondary candidate-track-info">
                            {candidateIdentitySummary(candidate) ||
                              'Provider candidate'}
                          </p>
                          {#if candidateFileSummary(candidate)}
                            <p class="candidate-file-info">
                              {candidateFileSummary(candidate)}
                            </p>
                          {/if}
                          {#if candidateIdentifiers(candidate)}
                            <p class="candidate-identifiers">
                              {candidateIdentifiers(candidate)}
                            </p>
                          {/if}
                          <div class="staging-candidate-actions">
                            <button
                              type="button"
                              class="btn btn-primary"
                              onclick={() =>
                                onResolve?.(
                                  selected.jobId!,
                                  group.provider,
                                  candidate.providerToken,
                                )}
                            >
                              <Icon name="download" size={13} strokeWidth={2} />
                              <span>Add</span>
                            </button>
                            <button
                              type="button"
                              class="btn candidate-reject"
                              onclick={() =>
                                onReject?.(
                                  selected.jobId!,
                                  group.provider,
                                  candidate.providerToken,
                                )}
                            >
                              <Icon name="trash" size={13} strokeWidth={2} />
                              <span>Reject</span>
                            </button>
                          </div>
                        </article>
                      {/each}
                    </section>
                  {/each}
                </div>
              </div>
            {/if}
          {:else}
            <div class="inspector-empty">
              <strong>Select a track</strong>
              <p>Artwork, metadata, provider state, and errors appear here.</p>
            </div>
          {/if}
        </aside>
      </div>
    {/if}

    {#if activeTransfer}
      {@const percent = progressPercent(activeTransfer)}
      <footer class="transfer-dock" aria-label="Current download progress">
        <div class="dock-track">
          <div class="dock-artwork artwork-fallback">
            {#if activeTransfer.imageUrl}
              <img src={activeTransfer.imageUrl} alt="" />
            {:else}
              <span>{activeTransfer.title.slice(0, 1).toUpperCase()}</span>
            {/if}
          </div>
          <div>
            <strong>{activeTransfer.title}</strong>
            <span>{activeTransfer.artists.join(', ') || 'Unknown artist'}</span>
          </div>
        </div>
        <div class="dock-progress-wrap">
          <div class:indeterminate={percent === null} class="dock-progress">
            <span style={percent === null ? '' : `width:${percent}%`}></span>
          </div>
          <div class="dock-progress-meta">
            <span>{percent === null ? 'Downloading…' : `${percent}%`}</span>
            <span
              >{formatBytes(activeTransfer.bytesTransferred)} / {formatBytes(
                activeTransfer.totalBytes,
              )}</span
            >
          </div>
        </div>
        <span class="dock-format"
          >{activeTransfer.candidate?.format?.toUpperCase() ?? 'LOSSLESS'}</span
        >
      </footer>
    {:else if syncBusy}
      <footer
        class="transfer-dock sync-dock"
        aria-label="Current synchronization progress"
      >
        <div class="dock-track">
          <div class="sync-pulse"></div>
          <div>
            <strong
              >{syncRun
                ? phaseLabel(syncRun.phase)
                : syncScope === 'local'
                  ? 'Reconciling local files'
                  : 'Preparing Spotify sync'}</strong
            >
            <span>Download → verify → import.</span>
          </div>
        </div>
        <div class="dock-progress-wrap">
          <div class="dock-progress indeterminate"><span></span></div>
        </div>
        <button type="button" class="btn" onclick={() => onCancelSync?.()}
          >Cancel</button
        >
      </footer>
    {/if}
  </div>
</div>

<style>
  .staging-screen {
    min-height: 0;
    padding-bottom: 20px;
    -webkit-user-select: none;
    user-select: none;
  }
  .staging-screen input {
    -webkit-user-select: text;
    user-select: text;
  }
  .staging-header {
    padding-bottom: 12px;
  }
  .staging-header-status {
    display: flex;
    align-items: center;
    gap: 14px;
    color: var(--text-secondary);
    font-size: 10px;
    font-variant-numeric: tabular-nums;
  }
  .staging-header-status strong {
    color: var(--text-primary);
    font-size: 11px;
    font-weight: 650;
  }
  .staging-error {
    flex: 0 0 auto;
    min-height: 0;
    margin-bottom: 10px;
    padding: 10px 12px;
  }
  .staging-shell {
    display: grid;
    min-height: 0;
    flex: 1;
    grid-template-rows: minmax(0, 1fr);
    grid-auto-rows: auto;
    gap: 10px;
  }
  .staging-loading {
    display: grid;
    align-content: start;
    gap: 8px;
  }
  .staging-main {
    display: grid;
    min-height: 0;
    grid-template-columns: minmax(0, 1fr) 308px;
    gap: 12px;
  }
  .queue-panel,
  .track-inspector,
  .sync-primary-card,
  .sync-summary-card {
    min-width: 0;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-panel);
    background: var(--bg-elevated);
  }
  .queue-panel {
    display: flex;
    min-height: 0;
    flex-direction: column;
    overflow: hidden;
  }
  .queue-toolbar {
    display: grid;
    grid-template-columns: 28px minmax(180px, 1fr) 150px auto;
    align-items: center;
    gap: 8px;
    border-bottom: 1px solid var(--border-subtle);
    padding: 12px 12px 13px;
  }
  .select-all,
  .card-checkbox {
    display: grid;
    place-items: center;
  }
  .card-checkbox {
    height: 48px;
    align-self: start;
  }
  .select-all input,
  .card-checkbox input {
    width: 14px;
    height: 14px;
    margin: 0;
  }
  .queue-filter {
    min-width: 0;
  }
  .queue-count {
    color: var(--text-tertiary);
    font-size: 10px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .selection-toolbar {
    display: flex;
    min-height: 44px;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    border-bottom: 1px solid var(--border-default);
    background: var(--blue-050);
    padding: 6px 10px 6px 14px;
  }
  .selection-toolbar > strong {
    color: var(--selection-strong-text);
    font-size: 11px;
    font-weight: 650;
  }
  .selection-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .text-button {
    border: 0;
    background: transparent;
    padding: 5px 7px;
    color: var(--blue-600);
    font-size: 10px;
    font-weight: 600;
    cursor: pointer;
  }
  .text-button:hover {
    color: var(--blue-700);
    text-decoration: underline;
  }
  .download-list {
    display: grid;
    min-height: 0;
    flex: 1;
    align-content: start;
    gap: 8px;
    overflow-y: auto;
    padding: 10px;
  }
  .download-card {
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr);
    align-items: start;
    gap: 10px;
    min-height: 96px;
    content-visibility: auto;
    contain-intrinsic-size: 82px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-card);
    background: var(--bg-content);
    padding: 10px;
    cursor: default;
    transition:
      border-color 120ms ease,
      background 120ms ease;
  }
  .download-card-main {
    display: grid;
    width: 100%;
    min-width: 0;
    grid-template-columns: 48px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    border: 0;
    background: transparent;
    padding: 0;
    color: inherit;
    text-align: left;
    cursor: default;
  }
  .download-card-content {
    display: grid;
    min-width: 0;
    gap: 7px;
  }
  .download-card:hover {
    border-color: var(--border-default);
    background: var(--bg-subtle);
  }
  .download-card.selected-card {
    border-color: var(--border-strong);
    background: var(--bg-subtle);
  }
  .download-artwork,
  .dock-artwork,
  .inspector-artwork {
    position: relative;
    display: grid;
    place-items: center;
    overflow: hidden;
    background: #e9edf2;
    color: var(--text-tertiary);
    font-weight: 650;
  }
  .download-artwork {
    width: 48px;
    height: 48px;
    border-radius: 7px;
  }
  .download-artwork img,
  .dock-artwork img,
  .inspector-artwork img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .download-copy {
    display: grid;
    min-width: 0;
    gap: 3px;
    padding-block: 1px;
  }
  .download-title-row {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 8px;
  }
  .download-title-row .track-title {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .download-copy .track-secondary,
  .download-album {
    margin: 0;
    overflow: hidden;
    color: var(--text-secondary);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .download-album {
    color: var(--text-tertiary);
  }
  .download-activity {
    min-width: 0;
    margin: 0 0 1px 58px;
    color: var(--text-secondary);
    font-size: 9.5px;
    line-height: 1.45;
    overflow-wrap: anywhere;
    white-space: normal;
  }
  .staging-exclude-selected {
    white-space: nowrap;
  }
  .download-meta {
    display: grid;
    justify-items: end;
    gap: 3px;
    min-width: 48px;
    color: var(--text-tertiary);
    font-size: 9px;
  }
  .download-meta strong {
    color: var(--text-primary);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }
  .download-provider {
    color: var(--text-tertiary);
    font-size: 8px;
  }
  .download-quality {
    color: var(--text-secondary);
    font-size: 8px;
    white-space: nowrap;
  }
  .card-progress,
  .dock-progress {
    position: relative;
    overflow: hidden;
    border-radius: 999px;
    background: var(--control-track);
  }
  .card-progress {
    height: 3px;
    margin: 7px 0 0 58px;
  }
  .card-progress span,
  .dock-progress span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--blue-600);
    transition: width 180ms ease;
  }
  .track-inspector {
    min-height: 0;
    overflow-y: auto;
    padding: 14px;
  }
  .track-inspector.resolution-inspector {
    display: flex;
    overflow: hidden;
    flex-direction: column;
  }
  .inspector-summary {
    min-width: 0;
  }
  .resolution-inspector .inspector-summary {
    display: grid;
    grid-template-columns: 64px minmax(0, 1fr);
    align-items: start;
    gap: 12px;
    border-bottom: 1px solid var(--border-subtle);
    padding-bottom: 10px;
  }
  .inspector-artwork {
    width: min(100%, 220px);
    aspect-ratio: 1;
    margin: 0 auto 14px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-card);
    font-size: 34px;
  }
  .resolution-inspector .inspector-artwork {
    width: 64px;
    height: 64px;
    aspect-ratio: auto;
    margin: 0;
    font-size: 22px;
  }
  .inspector-heading {
    border-bottom: 1px solid var(--border-subtle);
    padding-bottom: 14px;
  }
  .inspector-heading h2 {
    margin: 9px 0 3px;
    font-size: 18px;
    font-weight: 680;
    letter-spacing: -0.025em;
    line-height: 1.15;
  }
  .inspector-heading p {
    margin: 0;
    color: var(--text-secondary);
    font-size: 11px;
  }
  .resolution-inspector .inspector-heading {
    min-width: 0;
    border-bottom: 0;
    padding-bottom: 0;
  }
  .resolution-inspector .inspector-heading h2 {
    margin-top: 6px;
    font-size: 15px;
    overflow-wrap: anywhere;
    word-break: normal;
  }
  .inspector-facts,
  .sync-run-facts {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0;
    margin: 0;
    padding: 7px 0;
  }
  .inspector-facts div,
  .sync-run-facts div {
    min-width: 0;
    padding: 8px 0;
  }
  .resolution-inspector .inspector-facts {
    grid-template-columns: repeat(3, minmax(0, 1fr));
    padding: 4px 0;
  }
  .resolution-inspector .inspector-facts div {
    padding: 5px 0;
  }
  .resolution-inspector .inspector-facts div:nth-child(3),
  .resolution-inspector .inspector-facts div:nth-child(5),
  .resolution-inspector .inspector-facts div:nth-child(6) {
    display: none;
  }
  .inspector-facts dt,
  .sync-run-facts dt {
    color: var(--text-tertiary);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.045em;
    text-transform: uppercase;
  }
  .inspector-facts dd,
  .sync-run-facts dd {
    margin: 3px 0 0;
    overflow: hidden;
    color: var(--text-primary);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .inspector-path {
    display: grid;
    gap: 5px;
    border-top: 1px solid var(--border-subtle);
    padding: 12px 0;
  }
  .inspector-path > span {
    color: var(--text-tertiary);
    font-size: 9px;
    font-weight: 600;
    text-transform: uppercase;
  }
  .inspector-path code {
    display: block;
    max-width: 100%;
    color: var(--text-secondary);
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 9px;
    line-height: 1.45;
    overflow-wrap: anywhere;
    white-space: normal;
  }
  .inspector-error {
    margin-top: 8px;
    border: 1px solid rgb(229 72 77 / 22%);
    border-radius: var(--radius-control);
    background: var(--red-100);
    padding: 10px;
    color: var(--red-600);
    font-size: 10px;
  }
  .inspector-error p {
    margin: 4px 0 0;
    line-height: 1.4;
  }
  .inspector-retry {
    min-height: 28px;
    margin-top: 9px;
    padding: 5px 10px;
    font-size: 10px;
  }
  .inspector-empty {
    display: grid;
    min-height: 200px;
    place-content: center;
    color: var(--text-secondary);
    text-align: center;
  }
  .inspector-empty p {
    max-width: 220px;
    margin: 5px 0 0;
    font-size: 10px;
  }
  .resolution-section {
    border-top: 1px solid var(--border-subtle);
    padding-top: 12px;
  }
  .resolution-inspector .resolution-section {
    display: flex;
    min-height: 0;
    flex: 1;
    flex-direction: column;
  }
  .resolution-heading {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
  }
  .resolution-heading h3 {
    margin: 0;
    font-size: 11px;
  }
  .resolution-heading p {
    margin: 3px 0 0;
    color: var(--text-secondary);
    font-size: 9px;
    line-height: 1.4;
  }
  .resolution-search {
    display: inline-flex;
    flex: 0 0 auto;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
  }
  .candidate-stack {
    display: grid;
    gap: 7px;
    margin-top: 9px;
  }
  .candidate-provider-group {
    display: grid;
    gap: 7px;
  }
  .candidate-provider-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 2px 2px 0;
    color: var(--text-secondary);
    font-size: 9px;
  }
  .candidate-provider-heading strong {
    color: var(--text-primary);
    font-size: 10px;
  }
  .resolution-inspector .candidate-stack {
    min-height: 0;
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    gap: 6px;
    margin-top: 8px;
    padding-right: 2px;
    scrollbar-gutter: stable;
  }
  .staging-candidate-card {
    min-width: 0;
    padding: 11px 12px;
  }
  .candidate-card-heading {
    display: flex;
    min-width: 0;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .candidate-card-heading .track-title {
    min-width: 0;
    flex: 1;
    overflow-wrap: anywhere;
    white-space: normal;
  }
  .candidate-confidence {
    flex: 0 0 auto;
    border: 1px solid rgb(23 105 255 / 22%);
    border-radius: 999px;
    background: var(--blue-050);
    padding: 2px 6px;
    color: var(--blue-600);
    font-size: 9px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .candidate-identifiers {
    margin: 5px 0 0;
    overflow: hidden;
    color: var(--text-tertiary);
    font-size: 8px;
    line-height: 1.35;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .candidate-track-info,
  .candidate-file-info {
    margin: 5px 0 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .candidate-file-info {
    color: var(--text-secondary);
    font-size: 9px;
    font-variant-numeric: tabular-nums;
  }
  .staging-candidate-actions {
    display: flex;
    width: 100%;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 10px;
  }
  .staging-candidate-actions .btn {
    width: auto;
    min-width: 78px;
    min-height: 30px;
    flex: 0 0 auto;
    padding: 0 11px;
    border-radius: 8px;
    font-size: 10px;
    white-space: nowrap;
  }
  .staging-candidate-actions .btn-primary {
    min-width: 72px;
  }
  .candidate-reject {
    margin-left: auto;
    color: var(--text-secondary);
  }
  .sync-dashboard {
    display: grid;
    min-height: 0;
    grid-template-columns: minmax(0, 1fr) 300px;
    gap: 12px;
  }
  .sync-primary-card,
  .sync-summary-card {
    overflow-y: auto;
    padding: 18px;
  }
  .sync-card-header,
  .sync-summary-title {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 14px;
  }
  .eyebrow {
    margin: 0;
    color: var(--text-tertiary);
    font-size: 9px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .sync-card-header h2 {
    margin: 5px 0 4px;
    font-size: 20px;
    letter-spacing: -0.025em;
  }
  .sync-card-header p:last-child {
    max-width: 620px;
    margin: 0;
    color: var(--text-secondary);
    font-size: 11px;
    line-height: 1.5;
  }
  .sync-summary-title h3 {
    margin: 5px 0 0;
    font-size: 13px;
  }
  .sync-timeline {
    display: grid;
    gap: 0;
    margin-top: 18px;
  }
  .sync-step {
    position: relative;
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr);
    gap: 9px;
    min-height: 48px;
    color: var(--text-tertiary);
  }
  .sync-step::after {
    position: absolute;
    top: 17px;
    bottom: -1px;
    left: 5px;
    width: 1px;
    background: var(--border-default);
    content: '';
  }
  .sync-step:last-child::after {
    display: none;
  }
  .sync-step-dot {
    position: relative;
    z-index: 1;
    width: 11px;
    height: 11px;
    margin-top: 2px;
    border: 2px solid var(--border-strong);
    border-radius: 999px;
    background: var(--bg-elevated);
  }
  .sync-step strong {
    display: block;
    color: inherit;
    font-size: 11px;
    font-weight: 600;
  }
  .sync-step small {
    display: block;
    margin-top: 2px;
    font-size: 9px;
  }
  .sync-step.done,
  .sync-step.current {
    color: var(--text-primary);
  }
  .sync-step.done .sync-step-dot {
    border-color: var(--green-600);
    background: var(--green-600);
  }
  .sync-step.current .sync-step-dot {
    border-color: var(--blue-600);
    box-shadow: 0 0 0 3px rgb(23 105 255 / 12%);
  }
  .sync-run-facts {
    margin-top: 10px;
    border-top: 1px solid var(--border-subtle);
  }
  .transfer-dock {
    display: grid;
    grid-template-columns: minmax(210px, 0.8fr) minmax(260px, 1.5fr) auto;
    align-items: center;
    gap: 16px;
    min-height: 64px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-panel);
    background: var(--bg-elevated);
    padding: 9px 12px;
  }
  .dock-track {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 9px;
  }
  .dock-track > div:last-child {
    min-width: 0;
  }
  .dock-track strong,
  .dock-track span {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dock-track strong {
    font-size: 11px;
    font-weight: 650;
  }
  .dock-track span {
    margin-top: 2px;
    color: var(--text-secondary);
    font-size: 9px;
  }
  .dock-artwork {
    width: 38px;
    height: 38px;
    flex: 0 0 38px;
    border-radius: 6px;
  }
  .dock-progress-wrap {
    min-width: 0;
  }
  .dock-progress {
    height: 5px;
  }
  .dock-progress-meta {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    margin-top: 5px;
    color: var(--text-tertiary);
    font-size: 9px;
    font-variant-numeric: tabular-nums;
  }
  .dock-format {
    color: var(--text-secondary);
    font-size: 9px;
    font-weight: 650;
    letter-spacing: 0.05em;
  }
  .card-progress.indeterminate span,
  .dock-progress.indeterminate span {
    width: 28%;
    animation: staging-slide 1.15s ease-in-out infinite alternate;
  }
  .sync-pulse {
    width: 9px;
    height: 9px;
    flex: 0 0 9px;
    border-radius: 999px;
    background: var(--blue-600);
    box-shadow: 0 0 0 4px rgb(23 105 255 / 10%);
  }
  @keyframes staging-slide {
    from {
      transform: translateX(-18%);
    }
    to {
      transform: translateX(275%);
    }
  }
  @media (max-width: 1120px) {
    .staging-main,
    .sync-dashboard {
      grid-template-columns: minmax(0, 1fr) 270px;
    }
    .queue-toolbar {
      grid-template-columns: 28px minmax(160px, 1fr) 130px;
    }
    .queue-count {
      display: none;
    }
  }
  @media (max-width: 930px) {
    .staging-main,
    .sync-dashboard {
      grid-template-columns: minmax(0, 1fr);
    }
    .track-inspector,
    .sync-summary-card {
      display: none;
    }
    .transfer-dock {
      grid-template-columns: minmax(180px, 0.8fr) minmax(220px, 1.3fr) auto;
    }
  }
</style>
