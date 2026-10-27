<script lang="ts">
  import { onMount } from 'svelte';
  import FolderSync from '~icons/lucide/folder-sync';
  import Plus from '~icons/lucide/plus';
  import { chooseMirrorDestination } from '../lib/dialog';
  import {
    listMirrorRuns,
    listMirrorTargets,
    removeMirrorTarget,
    saveMirrorTarget,
    type MirrorProgress,
    type MirrorRun,
    type MirrorTarget,
  } from '../lib/mirror';

  export let libraryRoot: string;
  export let busyTarget: number | null = null;
  export let progress: MirrorProgress | null = null;
  export let timeEstimate = 'Estimating time…';
  export let transferSpeed: string | null = null;
  export let cancelling = false;
  export let cancelError: string | null = null;
  export let onRun: (target: MirrorTarget) => void | Promise<void>;
  export let onCancel: () => void | Promise<void>;

  let targets: MirrorTarget[] = [];
  let history: Record<number, MirrorRun[]> = {};
  let adding = false;
  let loading = true;
  let error: string | null = null;

  const date = (timestamp: number) =>
    new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(new Date(timestamp));

  const message = (reason: unknown) =>
    reason instanceof Error ? reason.message : String(reason);

  const pathParts = (path: string) =>
    path.replace(/[\\/]+$/, '').split(/[\\/]/);
  const destinationName = (path: string) => pathParts(path).at(-1) || path;
  const destinationFolder = (path: string) =>
    pathParts(path).slice(0, -1).join('/') || '/';
  const fileFolder = (path: string) => pathParts(path).slice(0, -1).join('/');
  const fileName = (path: string) => pathParts(path).at(-1) || path;
  const percent = (done: number, total: number) =>
    total > 0 ? Math.max(0, Math.min(100, (done / total) * 100)) : 0;
  const fileSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
    return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  };
  const stageSegments = Array.from({ length: 24 }, (_, index) => index);

  async function reload() {
    targets = await listMirrorTargets();
    const entries = await Promise.all(
      targets.map(
        async (target) => [target.id, await listMirrorRuns(target.id)] as const,
      ),
    );
    history = Object.fromEntries(entries);
  }

  onMount(() => {
    let mounted = true;
    void reload()
      .catch((reason) => {
        if (mounted) error = message(reason);
      })
      .finally(() => {
        if (mounted) loading = false;
      });
    const availabilityTimer = window.setInterval(() => {
      if (!mounted || busyTarget !== null || adding) return;
      void listMirrorTargets()
        .then((latest) => {
          if (mounted && !adding && busyTarget === null) targets = latest;
        })
        .catch(() => {});
    }, 5000);
    return () => {
      mounted = false;
      window.clearInterval(availabilityTimer);
    };
  });

  async function add() {
    try {
      error = null;
      const path = await chooseMirrorDestination();
      if (!path) return;
      adding = true;
      await saveMirrorTarget(path);
      await reload();
    } catch (reason) {
      error = message(reason);
    } finally {
      adding = false;
    }
  }

  async function remove(target: MirrorTarget) {
    error = null;
    try {
      await removeMirrorTarget(target.id);
      await reload();
    } catch (reason) {
      error = message(reason);
    }
  }
</script>

<section class="mirror-card" aria-label="Library mirrors">
  <div class="mirror-heading">
    <div>
      <h3>Library mirrors</h3>
      <p>Keep your indexed music and playlists in sync with another folder.</p>
    </div>
    <button
      type="button"
      class="mirror-primary"
      onclick={add}
      disabled={adding || busyTarget !== null || !libraryRoot.trim()}
    >
      <Plus aria-hidden="true" />
      {adding ? 'Adding…' : 'Add destination'}
    </button>
  </div>

  {#if error}
    <p class="mirror-error" role="alert">{error}</p>
  {/if}
  {#if loading}
    <p class="mirror-muted">Loading mirrors…</p>
  {:else if targets.length === 0}
    <p class="mirror-empty">
      No mirror destinations configured. Add an existing folder on a connected
      drive to create a portable library copy.
    </p>
  {:else}
    <div class="mirror-targets">
      {#each targets as target (target.id)}
        <div class="mirror-target">
          <div class="mirror-target-head">
            <div class="mirror-target-details">
              <span class="mirror-drive-icon" aria-hidden="true"
                ><FolderSync /></span
              >
              <div class="mirror-destination-copy" title={target.path}>
                <strong>{destinationName(target.path)}</strong>
                <span class="mirror-destination-path"
                  >{destinationFolder(target.path)}</span
                >
                {#if !target.available}
                  <span class="mirror-muted"
                    >Disconnected. Reconnect to resume.</span
                  >
                {/if}
              </div>
            </div>
            <div class="mirror-actions">
              {#if busyTarget === target.id}
                <span class="mirror-active-label"
                  ><span class="mirror-live-dot"></span> Active</span
                >
                <button
                  type="button"
                  class="mirror-secondary mirror-cancel"
                  onclick={() => void onCancel()}
                  disabled={cancelling}
                  >{cancelling ? 'Stopping…' : 'Cancel'}</button
                >
              {:else}
                <button
                  type="button"
                  class="mirror-primary"
                  onclick={() => {
                    error = null;
                    void onRun(target);
                  }}
                  disabled={busyTarget !== null || adding || !target.available}
                  >Mirror now</button
                >
                <button
                  type="button"
                  class="mirror-text-button"
                  onclick={() => remove(target)}
                  disabled={busyTarget !== null || adding}
                  title="Forget this destination without deleting its files"
                  >Remove</button
                >
              {/if}
            </div>
          </div>
          {#if busyTarget === target.id && progress}
            {@const stagePercent = percent(progress.completed, progress.total)}
            {@const filePercent = percent(
              progress.fileBytesDone,
              progress.fileBytesTotal,
            )}
            {@const timingParts = timeEstimate.split(' · ')}
            {@const hasElapsed = timeEstimate.startsWith('Elapsed ')}
            <div class="mirror-progress">
              <div class="mirror-task-heading" role="status">
                <div class="mirror-task-title">
                  <span class="mirror-task-eyebrow">MIRROR IN PROGRESS</span>
                  <strong
                    >{cancelling
                      ? 'Stopping mirror…'
                      : progress.operation || 'Preparing library…'}</strong
                  >
                </div>
                <span class="mirror-elapsed"
                  >{hasElapsed ? timingParts[0] : 'Starting…'}</span
                >
              </div>
              <div class="mirror-stage-section">
                <div class="mirror-meter-heading">
                  <span class="mirror-meter-label">Stage progress</span>
                  <span class="mirror-meter-value">
                    {progress.total > 0
                      ? `${Math.min(progress.completed, progress.total).toLocaleString()} / ${progress.total.toLocaleString()} items`
                      : 'Preparing…'}
                    {#if progress.total > 0}
                      <strong>{Math.round(stagePercent)}%</strong>
                    {/if}
                  </span>
                </div>
                <div
                  class="mirror-stage-meter"
                  role="progressbar"
                  aria-label="Items in current mirror stage"
                  aria-valuemin="0"
                  aria-valuemax={Math.max(1, progress.total)}
                  aria-valuenow={Math.max(
                    0,
                    Math.min(progress.completed, Math.max(1, progress.total)),
                  )}
                >
                  {#each stageSegments as segment (segment)}
                    <span
                      class="mirror-stage-segment"
                      style={`--segment-fill: ${Math.max(0, Math.min(100, stagePercent * 24 - segment * 100))}%`}
                    ></span>
                  {/each}
                </div>
                <span class="mirror-estimate"
                  >{hasElapsed
                    ? timingParts.slice(1).join(' · ') || 'Estimating time…'
                    : timeEstimate}</span
                >
              </div>
              {#if progress.currentFile}
                <div class="mirror-file-section">
                  <div class="mirror-file-heading">
                    <div class="mirror-file-identity">
                      <span class="mirror-meter-label">Current file</span>
                      <strong title={progress.currentFile}
                        >{fileName(progress.currentFile)}</strong
                      >
                    </div>
                    {#if progress.fileBytesTotal > 0}
                      <strong class="mirror-file-percent"
                        >{Math.round(filePercent)}%</strong
                      >
                    {/if}
                  </div>
                  {#if fileFolder(progress.currentFile)}
                    <span class="mirror-file-path" title={progress.currentFile}
                      >{fileFolder(progress.currentFile)}</span
                    >
                  {/if}
                  {#if progress.fileBytesTotal > 0}
                    <div
                      class="mirror-file-meter"
                      role="progressbar"
                      aria-label="Current file progress"
                      aria-valuemin="0"
                      aria-valuemax={progress.fileBytesTotal}
                      aria-valuenow={Math.max(
                        0,
                        Math.min(
                          progress.fileBytesDone,
                          progress.fileBytesTotal,
                        ),
                      )}
                    >
                      <span
                        class="mirror-file-meter-fill"
                        style={`width: ${filePercent}%`}
                      ></span>
                    </div>
                    <div class="mirror-file-meta">
                      <span class="mirror-byte-count">
                        {fileSize(progress.fileBytesDone)} of {fileSize(
                          progress.fileBytesTotal,
                        )} ·
                        {progress.operation === 'Copying file'
                          ? 'copied'
                          : progress.operation === 'Exporting playlist'
                            ? 'exported'
                            : progress.operation === 'Hashing source file'
                              ? 'hashed'
                              : 'checked'}
                      </span>
                      {#if progress.operation === 'Copying file'}
                        <strong class="mirror-transfer-speed">
                          Transfer speed: {transferSpeed ?? 'Calculating…'}
                        </strong>
                      {/if}
                    </div>
                  {/if}
                </div>
              {/if}
              <div class="mirror-result-counts" aria-label="Mirror file counts">
                <span><strong>{progress.copied}</strong> copied</span>
                <span><strong>{progress.unchanged}</strong> unchanged</span>
                <span><strong>{progress.removed}</strong> removed</span>
              </div>
              {#if cancelError}
                <p class="mirror-error" role="alert">{cancelError}</p>
              {/if}
            </div>
          {:else}
            <div class="mirror-last-run">
              {#if target.lastRun}
                <span class:mirror-failed={target.lastRun.status === 'failed'}>
                  Last run: {target.lastRun.status === 'succeeded'
                    ? 'Complete'
                    : target.lastRun.status === 'running'
                      ? 'Interrupted or still running'
                      : target.lastRun.status === 'cancelled'
                        ? 'Cancelled'
                        : 'Failed'} · {date(target.lastRun.startedAt)}
                </span>
              {:else}
                <span>Never mirrored</span>
              {/if}
            </div>
          {/if}
          {#if busyTarget !== target.id && target.lastRun?.status === 'succeeded'}
            <p class="mirror-muted">
              {target.lastRun.copied} copied · {target.lastRun.unchanged} unchanged
              · {target.lastRun.removed} removed
            </p>
          {:else if busyTarget !== target.id && target.lastRun?.status === 'cancelled'}
            <p class="mirror-muted">Cancelled. Run again to resume safely.</p>
          {:else if busyTarget !== target.id && target.lastRun?.error}
            <p class="mirror-error">{target.lastRun.error}</p>
          {/if}
          {#if (history[target.id]?.length ?? 0) > 1}
            <details>
              <summary>Recent mirror runs ({history[target.id].length})</summary
              >
              <ul>
                {#each history[target.id] as item (item.id)}
                  <li>
                    {date(item.startedAt)}: {item.status}, {item.copied} copied, {item.removed}
                    removed
                  </li>
                {/each}
              </ul>
            </details>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
  <p class="mirror-muted">
    Removing a destination keeps its files. Refrain only manages files it owns.
  </p>
</section>

<style>
  .mirror-card {
    border: 1px solid var(--border-default);
    border-radius: var(--radius-card);
    padding: 20px;
    background: var(--bg-elevated);
    display: grid;
    gap: 18px;
  }
  .mirror-heading,
  .mirror-target-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-width: 0;
  }
  .mirror-heading h3 {
    font-size: 14px;
    font-weight: 690;
    margin: 0 0 5px;
  }
  .mirror-heading p,
  .mirror-muted,
  .mirror-empty {
    color: var(--text-secondary);
    font-size: 12px;
    line-height: 1.55;
    margin: 0;
  }
  .mirror-empty {
    padding: 12px;
    border: 1px dashed var(--border-default);
    border-radius: 8px;
  }
  .mirror-primary,
  .mirror-secondary,
  .mirror-text-button {
    cursor: pointer;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-control);
    padding: 8px 12px;
    font-weight: 630;
    font-size: 12px;
    white-space: nowrap;
  }
  .mirror-primary {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    background: var(--action-primary-bg);
    border-color: var(--action-primary-bg);
    color: var(--action-primary-text);
  }
  .mirror-primary :global(svg) {
    width: 14px;
    height: 14px;
  }
  .mirror-secondary {
    color: var(--text-primary);
    background: var(--bg-control);
  }
  .mirror-text-button {
    color: var(--text-tertiary);
    background: transparent;
    border-color: transparent;
    font-weight: 560;
  }
  .mirror-text-button:hover:not(:disabled),
  .mirror-secondary:hover:not(:disabled) {
    background: var(--bg-hover);
  }
  button:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }
  button:focus-visible,
  summary:focus-visible {
    outline: 2px solid var(--focus-ring);
    outline-offset: 2px;
  }
  .mirror-targets {
    display: grid;
    gap: 18px;
  }
  .mirror-target {
    border-top: 1px solid var(--border-subtle);
    padding-top: 18px;
    display: grid;
    gap: 12px;
  }
  .mirror-target-details {
    display: flex;
    align-items: center;
    min-width: 0;
    flex: 1;
    gap: 11px;
  }
  .mirror-drive-icon {
    width: 38px;
    height: 38px;
    flex: 0 0 38px;
    display: grid;
    place-items: center;
    border: 1px solid var(--border-subtle);
    border-radius: 10px;
    background: var(--bg-subtle);
    color: var(--text-secondary);
  }
  .mirror-drive-icon :global(svg) {
    width: 19px;
    height: 19px;
  }
  .mirror-destination-copy {
    display: grid;
    gap: 3px;
    min-width: 0;
    flex: 1;
  }
  .mirror-destination-copy strong {
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 680;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mirror-destination-path {
    color: var(--text-tertiary);
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mirror-last-run,
  summary,
  li {
    color: var(--text-secondary);
    font-size: 11.5px;
  }
  .mirror-failed,
  .mirror-error {
    color: var(--red-600) !important;
  }
  .mirror-error {
    font-size: 12px;
    line-height: 1.5;
    overflow-wrap: anywhere;
    margin: 0;
  }
  .mirror-actions {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .mirror-active-label {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    margin-right: 6px;
    font-size: 11px;
    font-weight: 650;
    color: var(--blue-600);
  }
  .mirror-live-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: currentColor;
    box-shadow: 0 0 0 4px color-mix(in srgb, currentColor 11%, transparent);
  }
  .mirror-cancel {
    min-width: 74px;
  }
  .mirror-progress {
    --file-accent: #168c76;
    --file-accent-highlight: #53c9ac;
    display: grid;
    gap: 16px;
    padding: 18px;
    border: 1px solid
      color-mix(in srgb, var(--blue-600) 14%, var(--border-subtle));
    border-radius: 12px;
    background: color-mix(in srgb, var(--bg-subtle) 89%, var(--blue-100));
    font-size: 12px;
    color: var(--text-secondary);
  }
  :global(html[data-theme='dark']) .mirror-progress {
    --file-accent: #36bca0;
    --file-accent-highlight: #84e4c4;
  }
  .mirror-task-heading,
  .mirror-meter-heading,
  .mirror-file-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-width: 0;
  }
  .mirror-task-title {
    display: grid;
    min-width: 0;
    gap: 5px;
  }
  .mirror-task-eyebrow {
    font-size: 10px;
    font-weight: 750;
    letter-spacing: 0.11em;
    color: var(--blue-600);
  }
  .mirror-task-title strong {
    color: var(--text-primary);
    font-size: 15px;
    font-weight: 690;
    line-height: 1.3;
  }
  .mirror-elapsed {
    font-size: 11px;
    color: var(--text-secondary);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .mirror-stage-section {
    display: grid;
    gap: 9px;
  }
  .mirror-meter-label {
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 640;
  }
  .mirror-meter-value {
    display: flex;
    align-items: baseline;
    gap: 12px;
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
  }
  .mirror-meter-value strong {
    font-size: 13px;
    color: var(--blue-600);
  }
  .mirror-stage-meter {
    display: grid;
    grid-template-columns: repeat(24, minmax(0, 1fr));
    gap: 3px;
    height: 11px;
  }
  .mirror-stage-segment {
    overflow: hidden;
    border-radius: 3px;
    background: color-mix(in srgb, var(--border-strong) 48%, transparent);
  }
  .mirror-stage-segment::before {
    content: '';
    display: block;
    width: var(--segment-fill);
    height: 100%;
    border-radius: inherit;
    background: var(--blue-600);
    transition: width 200ms ease-out;
  }
  .mirror-estimate {
    min-height: 14px;
    font-size: 11px;
    color: var(--text-tertiary);
    font-variant-numeric: tabular-nums;
  }
  .mirror-file-section {
    display: grid;
    gap: 7px;
    min-width: 0;
    padding: 13px 14px;
    border: 1px solid var(--border-subtle);
    border-radius: 10px;
    background: var(--bg-elevated);
  }
  .mirror-file-identity {
    display: grid;
    min-width: 0;
    gap: 5px;
  }
  .mirror-file-identity strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-primary);
    font-size: 12.5px;
    font-weight: 650;
  }
  .mirror-file-percent {
    color: var(--file-accent);
    font-size: 15px;
    font-variant-numeric: tabular-nums;
  }
  .mirror-file-path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
    color: var(--text-tertiary);
  }
  .mirror-file-meter {
    margin-top: 5px;
    height: 14px;
    padding: 2px;
    border-radius: 100px;
    overflow: hidden;
    background: var(--control-track);
    box-shadow: inset 0 1px 2px rgb(0 0 0 / 7%);
  }
  .mirror-file-meter-fill {
    position: relative;
    display: block;
    height: 100%;
    min-width: 0;
    border-radius: inherit;
    background: linear-gradient(
      90deg,
      var(--file-accent),
      var(--file-accent-highlight)
    );
    box-shadow: 0 0 9px color-mix(in srgb, var(--file-accent) 30%, transparent);
    transition: width 200ms ease-out;
    overflow: hidden;
  }
  .mirror-file-meter-fill::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(
      105deg,
      transparent 20%,
      rgb(255 255 255 / 30%) 50%,
      transparent 80%
    );
    transform: translateX(-100%);
    animation: mirror-sheen 2.3s ease-in-out infinite;
  }
  .mirror-byte-count {
    font-size: 11px;
    color: var(--text-tertiary);
    font-variant-numeric: tabular-nums;
  }
  .mirror-file-meta {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: baseline;
    gap: 4px 12px;
  }
  .mirror-transfer-speed {
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }
  .mirror-result-counts {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 14px;
    padding-top: 1px;
    font-size: 11px;
    color: var(--text-tertiary);
    font-variant-numeric: tabular-nums;
  }
  .mirror-result-counts strong {
    color: var(--text-secondary);
    font-weight: 670;
  }
  @keyframes mirror-sheen {
    to {
      transform: translateX(100%);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .mirror-file-meter-fill::after {
      animation: none;
    }
    .mirror-stage-segment::before,
    .mirror-file-meter-fill {
      transition: none;
    }
  }
  summary {
    cursor: pointer;
  }
  ul {
    margin: 6px 0;
    padding-left: 20px;
  }
  @media (max-width: 650px) {
    .mirror-heading,
    .mirror-target-head {
      flex-wrap: wrap;
    }
    .mirror-card {
      padding: 14px;
    }
    .mirror-progress {
      padding: 14px;
    }
  }
</style>
