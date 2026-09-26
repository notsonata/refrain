<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { save } from '@tauri-apps/plugin-dialog';
  import Icon from './Icon.svelte';
  import OverflowMenu from './OverflowMenu.svelte';
  import TrackList from './TrackList.svelte';
  import type { OverflowMenuItem } from '../lib/menu';
  import type {
    SourceAlbumMetadata,
    SourceCollectionEntryView,
    SourceCollectionSummary,
    SpotifySourceOverview,
  } from '../lib/source';

  type SpotifySection = 'liked' | 'albums' | 'playlists';
  type TrackListSelectionController = {
    toggleSelectAllShown: () => void;
    clearSelection: () => void;
    applySelectedTracking: (included: boolean | null) => Promise<void>;
  };

  export let section: SpotifySection = 'liked';
  export let overview: SpotifySourceOverview | null = null;
  export let savedAlbums: SourceCollectionSummary[] = [];
  export let savedAlbumTotal = 0;
  export let playlists: SourceCollectionSummary[] = [];
  export let playlistTotal = 0;
  export let currentCollection: SourceCollectionSummary | null = null;
  export let entries: SourceCollectionEntryView[] = [];
  export let collectionTotal = 0;
  export let collectionLoading = false;
  export let collectionLoadingMore = false;
  export let collectionError: string | null = null;
  export let savedAlbumsLoadingMore = false;
  export let playlistsLoadingMore = false;
  export let trackingBusyId: number | null = null;
  export let trackingBulkBusy = false;

  export let onSelectSavedAlbum:
    ((album: SourceCollectionSummary) => void) | undefined = undefined;
  export let onSelectPlaylist:
    ((playlist: SourceCollectionSummary) => void) | undefined = undefined;
  export let onBackToCollections: (() => void) | undefined = undefined;
  export let onLoadMoreSavedAlbums: (() => void) | undefined = undefined;
  export let onLoadMorePlaylists: (() => void) | undefined = undefined;
  export let onLoadMoreCollection: (() => void) | undefined = undefined;
  export let onSetCollectionTracking:
    | ((collection: SourceCollectionSummary, included: boolean) => void)
    | undefined = undefined;
  export let onSetTrackTracking:
    | ((
        entry: SourceCollectionEntryView,
        included: boolean | null,
      ) => void | Promise<void>)
    | undefined = undefined;
  export let onSetTracksTracking:
    | ((
        entries: SourceCollectionEntryView[],
        included: boolean | null,
      ) => void | Promise<void>)
    | undefined = undefined;

  let collectionSearch = '';
  let collectionState = 'all';
  let collectionFiltersOpen = false;
  let collectionLoadAllRequested = false;
  let lastCollectionAutoLoadCount = -1;
  let lastCollectionAutoLoadSection: SpotifySection | null = null;
  let collectionActionError: string | null = null;
  let coverDownloading = false;
  let detailsSidebarOpen = true;
  let trackListController: TrackListSelectionController | undefined;
  let selectedTrackCount = 0;
  let selectableTrackCount = 0;
  let allShownSelected = false;

  $: accessiblePlaylists = playlists.filter(
    (playlist) => playlist.isAccessible,
  );
  $: unavailablePlaylists = playlists.filter(
    (playlist) => !playlist.isAccessible,
  );
  $: collectionList = section === 'albums' ? savedAlbums : accessiblePlaylists;
  $: filteredCollections = collectionList.filter((collection) => {
    const query = collectionSearch.trim().toLocaleLowerCase();
    if (
      query &&
      ![collection.name, ...(collection.albumMetadata?.artists ?? [])].some(
        (value) => value.toLocaleLowerCase().includes(query),
      )
    )
      return false;
    if (
      collectionState === 'missing' &&
      collection.localEntryCount >= collection.entryCount
    )
      return false;
    if (
      collectionState === 'attention' &&
      collection.attentionEntryCount <= 0 &&
      collection.isAccessible
    )
      return false;
    return true;
  });
  $: filteredUnavailablePlaylists =
    section === 'playlists' &&
    (collectionState === 'all' || collectionState === 'attention')
      ? unavailablePlaylists.filter((playlist) => {
          const query = collectionSearch.trim().toLocaleLowerCase();
          return !query || playlist.name.toLocaleLowerCase().includes(query);
        })
      : [];
  $: filteredCollectionCount =
    filteredCollections.length + filteredUnavailablePlaylists.length;
  $: displayedCollection =
    section === 'liked'
      ? currentCollection?.id === overview?.likedSongs?.id
        ? currentCollection
        : (overview?.likedSongs ?? null)
      : currentCollection;
  $: localCollections = collectionList.filter(
    (collection) =>
      collection.entryCount > 0 &&
      collection.localEntryCount >= collection.entryCount,
  ).length;
  $: notLocalCollections = collectionList.filter(
    (collection) => collection.localEntryCount < collection.entryCount,
  ).length;
  $: attentionCollections =
    collectionList.filter((collection) => collection.attentionEntryCount > 0)
      .length + (section === 'playlists' ? unavailablePlaylists.length : 0);
  $: collectionLoadedCount =
    section === 'albums'
      ? savedAlbums.length
      : section === 'playlists'
        ? playlists.length
        : 0;
  $: collectionListTotal =
    section === 'albums'
      ? savedAlbumTotal
      : section === 'playlists'
        ? playlistTotal
        : 0;
  $: collectionHasMore = collectionLoadedCount < collectionListTotal;
  $: collectionFilterActive =
    collectionSearch.trim() !== '' || collectionState !== 'all';
  $: collectionListLoadingMore =
    section === 'albums'
      ? savedAlbumsLoadingMore
      : section === 'playlists'
        ? playlistsLoadingMore
        : false;
  $: if (
    section !== 'liked' &&
    (collectionLoadAllRequested || collectionFilterActive) &&
    collectionHasMore &&
    !collectionListLoadingMore &&
    (section !== lastCollectionAutoLoadSection ||
      collectionLoadedCount !== lastCollectionAutoLoadCount)
  ) {
    lastCollectionAutoLoadSection = section;
    lastCollectionAutoLoadCount = collectionLoadedCount;
    if (section === 'albums') onLoadMoreSavedAlbums?.();
    if (section === 'playlists') onLoadMorePlaylists?.();
  }
  $: if (
    !(collectionLoadAllRequested || collectionFilterActive) ||
    !collectionHasMore
  ) {
    lastCollectionAutoLoadCount = -1;
    lastCollectionAutoLoadSection = null;
  }
  $: if (!collectionHasMore) collectionLoadAllRequested = false;
  $: albumOrganization = albumOrganizationInfo(
    displayedCollection?.albumMetadata,
  );
  $: albumDurationMs = entries.reduce(
    (total, entry) => total + (entry.track?.durationMs ?? 0),
    0,
  );
  $: albumDuration = formatAlbumDuration(albumDurationMs);

  function collectionStatus(collection: SourceCollectionSummary): string {
    if (collection.trackedEntryCount <= 0) return 'Untracked';
    return `${formatNumber(collection.trackedEntryCount)}/${formatNumber(collection.entryCount)} Tracked`;
  }

  function setCollectionState(value: 'all' | 'missing' | 'attention') {
    lastCollectionAutoLoadCount = -1;
    lastCollectionAutoLoadSection = null;
    collectionState = value;
    collectionLoadAllRequested = true;
  }

  function clearCollectionFilters() {
    setCollectionState('all');
  }

  function formatNumber(value: number): string {
    return new Intl.NumberFormat().format(value);
  }

  function formatAlbumType(value: string | null | undefined): string {
    if (!value) return '—';
    return value.charAt(0).toUpperCase() + value.slice(1).replaceAll('_', ' ');
  }

  function formatReleaseDate(value: string | null | undefined): string {
    if (!value) return '—';
    const parts = value.split('-').map(Number);
    if (parts.length === 1 || !parts[0]) return value;
    const date = new Date(
      Date.UTC(parts[0], (parts[1] ?? 1) - 1, parts[2] ?? 1),
    );
    if (Number.isNaN(date.getTime())) return value;
    if (parts.length === 2) {
      return new Intl.DateTimeFormat(undefined, {
        month: 'short',
        year: 'numeric',
        timeZone: 'UTC',
      }).format(date);
    }
    return new Intl.DateTimeFormat(undefined, {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
      timeZone: 'UTC',
    }).format(date);
  }

  function formatAlbumDuration(value: number): string {
    if (value <= 0) return '—';
    const totalMinutes = Math.round(value / 60_000);
    if (totalMinutes < 60) return `${totalMinutes} min`;
    const hours = Math.floor(totalMinutes / 60);
    const minutes = totalMinutes % 60;
    return minutes === 0 ? `${hours} hr` : `${hours} hr ${minutes} min`;
  }

  function albumOrganizationInfo(
    metadata: SourceAlbumMetadata | null | undefined,
  ): { value: string; label: string } {
    const label = metadata?.label?.trim();
    if (label) return { value: label, label: 'label' };

    const copyright =
      metadata?.copyrights.find((value) => value.trim().startsWith('℗')) ??
      metadata?.copyrights[0];
    const rightsHolder = copyright
      ?.trim()
      .replace(/^(?:©|℗|\(c\)|\(p\))\s*/i, '')
      .replace(/^\d{4}\s+/, '')
      .trim();

    return rightsHolder
      ? { value: rightsHolder, label: 'rights holder' }
      : { value: '—', label: 'rights holder' };
  }

  function safeCollectionFilename(value: string): string {
    const safe = value
      .trim()
      .replace(/[<>:"/\\|?*\u0000-\u001f]/g, '_')
      .replace(/\s+/g, ' ');
    return safe || 'collection-cover';
  }

  async function openCollectionInSpotify() {
    const url =
      displayedCollection?.externalUrl ??
      displayedCollection?.albumMetadata?.externalUrl;
    if (!url) return;
    collectionActionError = null;
    try {
      await invoke('open_external_url', { url });
    } catch (error) {
      collectionActionError = String(error);
    }
  }

  async function downloadCollectionCover() {
    const url = displayedCollection?.imageUrl;
    if (!url || coverDownloading) return;

    collectionActionError = null;
    try {
      const destination = await save({
        title: 'Save cover artwork',
        defaultPath: `${safeCollectionFilename(displayedCollection.name)}.jpg`,
        filters: [{ name: 'JPEG image', extensions: ['jpg', 'jpeg'] }],
      });
      if (!destination) return;

      coverDownloading = true;
      await invoke('download_remote_file', { url, destination });
    } catch (error) {
      collectionActionError = String(error);
    } finally {
      coverDownloading = false;
    }
  }

  async function copyText(value: string) {
    await navigator.clipboard.writeText(value);
  }

  function collectionMenuItems(): OverflowMenuItem[] {
    if (!displayedCollection) return [];
    const items: OverflowMenuItem[] = [];
    const externalUrl =
      displayedCollection.externalUrl ??
      displayedCollection.albumMetadata?.externalUrl;

    if (externalUrl) {
      items.push(
        {
          label: 'Open in Spotify',
          icon: 'link',
          action: openCollectionInSpotify,
        },
        {
          label: 'Copy Spotify Link',
          icon: 'copy',
          action: () => copyText(externalUrl),
        },
      );
    }
    if (displayedCollection.imageUrl) {
      items.push({
        label: 'Download Cover',
        icon: 'download',
        action: downloadCollectionCover,
        disabled: coverDownloading,
      });
    }

    return items;
  }

  function updateTrackSelectionState(state: {
    selectedCount: number;
    selectableFilteredCount: number;
    allFilteredSelected: boolean;
  }) {
    selectedTrackCount = state.selectedCount;
    selectableTrackCount = state.selectableFilteredCount;
    allShownSelected = state.allFilteredSelected;
  }

  function trackSelectionMenuItems(): OverflowMenuItem[] {
    return [
      {
        label: allShownSelected ? 'Clear All Shown' : 'Select All Shown',
        icon: allShownSelected ? 'close' : 'check',
        action: () => trackListController?.toggleSelectAllShown(),
        disabled: trackingBulkBusy || selectableTrackCount === 0,
      },
      {
        label: 'Track Selected',
        icon: 'check',
        action: () => trackListController?.applySelectedTracking(true),
        disabled: trackingBulkBusy || selectedTrackCount === 0,
      },
      {
        label: 'Exclude Selected',
        icon: 'close',
        action: () => trackListController?.applySelectedTracking(false),
        disabled: trackingBulkBusy || selectedTrackCount === 0,
      },
      {
        label: 'Use Default for Selected',
        icon: 'refresh',
        action: () => trackListController?.applySelectedTracking(null),
        disabled: trackingBulkBusy || selectedTrackCount === 0,
      },
      {
        label: 'Clear Selection',
        icon: 'close',
        action: () => trackListController?.clearSelection(),
        disabled: trackingBulkBusy || selectedTrackCount === 0,
      },
    ];
  }
</script>

<div
  class="screen"
  class:album-detail-screen={section !== 'liked' && !!displayedCollection}
  class:details-sidebar-open={section !== 'liked' &&
    !!displayedCollection &&
    detailsSidebarOpen}
  class:details-sidebar-closed={section !== 'liked' &&
    !!displayedCollection &&
    !detailsSidebarOpen}
>
  {#if section !== 'liked' && displayedCollection}
    <div
      class="toolbar detail-toolbar"
      style="padding-top:2px; padding-bottom:12px; border-bottom:1px solid var(--divider);"
    >
      <div class="detail-breadcrumb">
        <button
          type="button"
          class="icon-button"
          aria-label="Back to collections"
          onclick={() => onBackToCollections?.()}
        >
          <Icon name="back" size={17} />
        </button>
        <strong>Spotify</strong>
        <Icon name="chevron-right" size={13} />
        <span>{section === 'albums' ? 'Albums' : 'Playlists'}</span>
        <Icon name="chevron-right" size={13} />
        <span class="detail-breadcrumb-current" title={displayedCollection.name}
          >{displayedCollection.name}</span
        >
      </div>
      <div style="flex:1"></div>
      <div class="account-block">
        <div class="account-avatar">
          {(overview?.account?.displayName ?? 'S').slice(0, 1).toUpperCase()}
          {#if overview?.account?.imageUrl}
            <img src={overview.account.imageUrl} alt="" />
          {/if}
        </div>
        <div>
          <p class="account-name">
            {overview?.account?.displayName ?? 'Spotify'}
          </p>
        </div>
      </div>
    </div>

    <div
      class="collection-detail-shell album-detail-shell"
      class:details-sidebar-closed={!detailsSidebarOpen}
    >
      <button
        type="button"
        class="icon-button detail-sidebar-toggle"
        aria-label={detailsSidebarOpen
          ? 'Hide details sidebar'
          : 'Show details sidebar'}
        title={detailsSidebarOpen ? 'Hide details' : 'Show details'}
        onclick={() => (detailsSidebarOpen = !detailsSidebarOpen)}
      >
        <span
          class="detail-toggle-icon"
          class:hidden={!detailsSidebarOpen}
          aria-hidden="true"
        >
          <Icon name="panel-right-close" size={16} />
        </span>
        <span
          class="detail-toggle-icon"
          class:hidden={detailsSidebarOpen}
          aria-hidden="true"
        >
          <Icon name="panel-right-open" size={16} />
        </span>
      </button>

      <div class="collection-detail-main">
        <div class="album-detail-header">
          <div class="album-detail-title-row">
            <h1 class="album-detail-title">{displayedCollection.name}</h1>
          </div>

          <div class="album-detail-meta-row">
            <div
              class="album-status-strip"
              aria-label={section === 'albums'
                ? 'Album status'
                : 'Playlist status'}
            >
              <div
                class="album-status-item"
                class:success={displayedCollection.trackedEntryCount > 0}
              >
                {#if displayedCollection.trackedEntryCount > 0}
                  <Icon name="check" size={14} />
                {/if}
                <strong>{collectionStatus(displayedCollection)}</strong>
              </div>
              <div class="album-status-item">
                <Icon name="cloud" size={14} />
                <strong
                  >{formatNumber(
                    Math.max(
                      0,
                      displayedCollection.entryCount -
                        displayedCollection.localEntryCount,
                    ),
                  )}</strong
                >
                <span>Spotify Only</span>
              </div>
              <div
                class="album-status-item"
                class:attention={displayedCollection.attentionEntryCount > 0}
              >
                <Icon name="warning" size={14} />
                <strong
                  >{formatNumber(
                    displayedCollection.attentionEntryCount,
                  )}</strong
                >
                <span>Needs Local Copy</span>
              </div>
            </div>

            <div class="album-detail-actions">
              <div class="collection-tracking-control">
                <span class="liked-tracking-label">
                  {section === 'albums' ? 'Track Album' : 'Track Playlist'}
                </span>
                <button
                  type="button"
                  class="tracking-switch"
                  class:active={displayedCollection.trackedByDefault}
                  aria-label={section === 'albums'
                    ? 'Track Album'
                    : 'Track Playlist'}
                  aria-pressed={displayedCollection.trackedByDefault}
                  disabled={trackingBusyId === -displayedCollection.id}
                  onclick={() =>
                    onSetCollectionTracking?.(
                      displayedCollection!,
                      !displayedCollection!.trackedByDefault,
                    )}
                >
                  <span class="tracking-switch-thumb"></span>
                </button>
              </div>
              <OverflowMenu
                items={trackSelectionMenuItems()}
                ariaLabel={`${displayedCollection.name} selection actions`}
              />
            </div>
          </div>
        </div>

        {#if collectionError}<div
            class="error-state"
            style="margin-bottom:8px;"
          >
            {collectionError}
          </div>{/if}
        {#key displayedCollection.id}
          <TrackList
            bind:this={trackListController}
            onSelectionChange={updateTrackSelectionState}
            {entries}
            total={collectionTotal}
            loading={collectionLoading}
            loadingMore={collectionLoadingMore}
            trackingControls={true}
            {trackingBusyId}
            {trackingBulkBusy}
            emptyMessage="This collection has no imported tracks."
            onLoadMore={onLoadMoreCollection}
            onSetTracking={onSetTrackTracking}
            onSetTrackingMany={onSetTracksTracking}
          />
        {/key}
      </div>

      <aside
        class="album-detail-inspector"
        class:closed={!detailsSidebarOpen}
        aria-hidden={!detailsSidebarOpen}
        inert={!detailsSidebarOpen}
      >
        <section class="album-inspector-section">
          <div class="album-inspector-heading">
            <h2 class="album-inspector-title">Details</h2>
            <div class="album-inspector-actions">
              <OverflowMenu
                items={collectionMenuItems()}
                ariaLabel={`${displayedCollection.name} details actions`}
              />
            </div>
          </div>
          <div class="album-info-list">
            {#if section === 'albums'}
              <div class="album-info-row">
                <span>Artist</span>
                <strong>
                  {displayedCollection.albumMetadata?.artists.length
                    ? displayedCollection.albumMetadata.artists.join(', ')
                    : '—'}
                </strong>
              </div>
              <div class="album-info-row">
                <span>Released</span>
                <strong
                  >{formatReleaseDate(
                    displayedCollection.albumMetadata?.releaseDate,
                  )}</strong
                >
              </div>
              <div class="album-info-row">
                <span>Type</span>
                <strong
                  >{formatAlbumType(
                    displayedCollection.albumMetadata?.albumType,
                  )}</strong
                >
              </div>
              <div class="album-info-row">
                <span
                  >{albumOrganization.label === 'label'
                    ? 'Label'
                    : 'Rights holder'}</span
                >
                <strong>{albumOrganization.value}</strong>
              </div>
            {/if}
            <div class="album-info-row compact-pair">
              <span>Tracks</span>
              <strong>{formatNumber(displayedCollection.entryCount)}</strong>
            </div>
            <div class="album-info-row compact-pair">
              <span>Duration</span>
              <strong>{albumDuration}</strong>
            </div>
            {#if section === 'playlists'}
              <div class="album-info-row">
                <span>Tracking</span>
                <strong>{collectionStatus(displayedCollection)}</strong>
              </div>
              <div class="album-info-row">
                <span>Default</span>
                <strong
                  >{displayedCollection.trackedByDefault
                    ? 'Tracked'
                    : 'Untracked'}</strong
                >
              </div>
              <div class="album-info-row">
                <span>Spotify</span>
                <strong
                  >{displayedCollection.isAccessible
                    ? 'Available'
                    : 'Unavailable'}</strong
                >
              </div>
            {/if}
          </div>
          {#if section === 'albums' && displayedCollection.albumMetadata?.copyrights.length}
            <p class="album-copyrights">
              {displayedCollection.albumMetadata.copyrights.join(' · ')}
            </p>
          {/if}
        </section>

        <section class="album-inspector-section album-cover-section">
          <div class="album-cover-panel">
            {#if displayedCollection.imageUrl}
              <img src={displayedCollection.imageUrl} alt="" />
            {:else}
              <div class="artwork-fallback album-cover-fallback">
                {displayedCollection.name.slice(0, 1).toUpperCase()}
              </div>
            {/if}
            {#if collectionActionError}
              <p class="album-action-error">{collectionActionError}</p>
            {/if}
          </div>
        </section>
      </aside>
    </div>
  {:else}
    <header class="page-header">
      <div class="spotify-heading">
        <div class="spotify-mark">
          <Icon name="spotify" size={25} strokeWidth={1.6} />
        </div>
        <div>
          <div class="page-heading-line">
            <h1 class="page-title">Spotify Library</h1>
          </div>
          <p class="page-description">
            {section === 'liked'
              ? 'Liked Songs'
              : section === 'albums'
                ? 'Albums'
                : 'Playlists'}
          </p>
        </div>
      </div>
      <div class="page-actions">
        <div class="account-block">
          <div class="account-avatar">
            {(overview?.account?.displayName ?? 'S').slice(0, 1).toUpperCase()}
            {#if overview?.account?.imageUrl}
              <img src={overview.account.imageUrl} alt="" />
            {/if}
          </div>
          <div>
            <p class="account-name">
              {overview?.account?.displayName ?? 'Spotify account'}
            </p>
          </div>
        </div>
      </div>
    </header>

    {#if section === 'liked'}
      {#if !displayedCollection}
        <div class="empty-state">Refresh Spotify to load Liked Songs.</div>
      {:else}
        <div class="metrics-strip spotify-library-metrics">
          <div class="metric-inline">
            <div class="metric-value">
              {formatNumber(displayedCollection.entryCount)}
            </div>
            <div class="metric-label">Total Tracks</div>
          </div>
          <div class="metric-inline">
            <div class="metric-value">
              {formatNumber(displayedCollection.localEntryCount)}
            </div>
            <div class="metric-label">Local</div>
          </div>
          <div class="metric-inline">
            <div class="metric-value">
              {formatNumber(
                Math.max(
                  0,
                  displayedCollection.entryCount -
                    displayedCollection.localEntryCount,
                ),
              )}
            </div>
            <div class="metric-label">Spotify Only</div>
          </div>
          <div class="metric-inline attention">
            <div class="metric-value">
              {formatNumber(displayedCollection.attentionEntryCount)}
            </div>
            <div class="metric-label">Needs Local Copy</div>
          </div>
          <div class="liked-tracking-row">
            <span class="liked-tracking-label">Track Liked Songs</span>
            <button
              type="button"
              class="tracking-switch"
              class:active={displayedCollection.trackedByDefault}
              aria-label="Track Liked Songs"
              aria-pressed={displayedCollection.trackedByDefault}
              disabled={trackingBusyId === -displayedCollection.id}
              onclick={() =>
                onSetCollectionTracking?.(
                  displayedCollection!,
                  !displayedCollection!.trackedByDefault,
                )}
            >
              <span class="tracking-switch-thumb"></span>
            </button>
            <OverflowMenu
              items={trackSelectionMenuItems()}
              ariaLabel="Liked Songs selection actions"
            />
          </div>
        </div>
        {#if collectionError}<div
            class="error-state"
            style="margin-bottom:8px;"
          >
            {collectionError}
          </div>{/if}
        {#key displayedCollection.id}
          <TrackList
            bind:this={trackListController}
            onSelectionChange={updateTrackSelectionState}
            {entries}
            total={collectionTotal}
            loading={collectionLoading}
            loadingMore={collectionLoadingMore}
            trackingControls={true}
            {trackingBusyId}
            {trackingBulkBusy}
            emptyMessage="No Liked Songs are currently imported."
            onLoadMore={onLoadMoreCollection}
            onSetTracking={onSetTrackTracking}
            onSetTrackingMany={onSetTracksTracking}
          />
        {/key}
      {/if}
    {:else}
      <div class="metrics-strip spotify-library-metrics">
        <div class="metric-inline">
          <div class="metric-value">
            {formatNumber(
              section === 'albums' ? savedAlbumTotal : playlistTotal,
            )}
          </div>
          <div class="metric-label">total {section}</div>
        </div>
        <div class="metric-inline">
          <div class="metric-value">{formatNumber(localCollections)}</div>
          <div class="metric-label">Local</div>
        </div>
        <div class="metric-inline">
          <div class="metric-value">{formatNumber(notLocalCollections)}</div>
          <div class="metric-label">Spotify Only</div>
        </div>
        <div class="metric-inline attention">
          <div class="metric-value">{formatNumber(attentionCollections)}</div>
          <div class="metric-label">Needs Local Copy</div>
        </div>
      </div>

      <div class="toolbar spotify-library-toolbar">
        <label class="search-field">
          <Icon name="search" size={17} />
          <input
            bind:value={collectionSearch}
            class="search-input"
            aria-label={`Search ${section}`}
            placeholder={`Search ${section === 'albums' ? 'albums or artists' : 'playlists or tracks'}…`}
          />
        </label>
        <button
          type="button"
          class="btn filter-button"
          class:filters-active={collectionState !== 'all'}
          aria-expanded={collectionFiltersOpen}
          onclick={() => (collectionFiltersOpen = !collectionFiltersOpen)}
        >
          <Icon name="filter" size={15} />
          Filters
          {#if collectionState !== 'all'}
            <span class="filter-count">1</span>
          {/if}
          <Icon name="chevron-down" size={13} />
        </button>
      </div>

      {#if collectionFiltersOpen}
        <div class="filters-panel collection-filters-panel">
          <div class="filters-panel-header">
            <div class="filters-panel-title">Filter results</div>
            <button
              type="button"
              class="link-button"
              onclick={clearCollectionFilters}>Clear</button
            >
          </div>
          <div class="filters-grid">
            <label class="filter-field">
              <span class="filter-label">Local Availability</span>
              <select
                value={collectionState}
                aria-label="Local state"
                class="filter-select"
                onchange={(event) =>
                  setCollectionState(
                    event.currentTarget.value as
                      'all' | 'missing' | 'attention',
                  )}
              >
                <option value="all">Any availability</option>
                <option value="missing">Spotify Only</option>
                <option value="attention">Needs Local Copy</option>
              </select>
            </label>
          </div>
        </div>
      {/if}

      <div class="collection-layout">
        <div class="collection-scroll">
          {#if collectionLoadedCount === 0}
            <div class="empty-state">
              No {section === 'albums' ? 'saved albums' : 'playlists'} imported yet.
            </div>
          {:else if filteredCollectionCount === 0}
            <div class="empty-state">
              No collections match the current filters.
            </div>
          {:else if filteredCollections.length > 0}
            <div class="collection-grid">
              {#each filteredCollections as collection (collection.id)}
                <button
                  type="button"
                  class="collection-card"
                  onclick={() =>
                    section === 'albums'
                      ? onSelectSavedAlbum?.(collection)
                      : onSelectPlaylist?.(collection)}
                >
                  <div class="collection-artwork">
                    {#if collection.imageUrl}
                      <img src={collection.imageUrl} alt="" loading="lazy" />
                    {:else}
                      <div
                        class="artwork-fallback"
                        style="width:100%; height:100%; font-size:26px;"
                      >
                        {collection.name.slice(0, 1).toUpperCase()}
                      </div>
                    {/if}
                  </div>
                  <p class="collection-name">{collection.name}</p>
                  <div class="collection-meta">
                    <span>{formatNumber(collection.entryCount)} tracks</span>
                    <span
                      class:success={collection.trackedEntryCount > 0}
                      class="chip">{collectionStatus(collection)}</span
                    >
                  </div>
                </button>
              {/each}
            </div>
          {/if}

          {#if section === 'albums' && savedAlbums.length < savedAlbumTotal}
            <div
              style="display:flex; justify-content:center; padding:18px 0 4px;"
            >
              <button
                type="button"
                class="btn"
                onclick={() => onLoadMoreSavedAlbums?.()}
                disabled={savedAlbumsLoadingMore}
                >{savedAlbumsLoadingMore
                  ? 'Loading…'
                  : 'Load More Albums'}</button
              >
            </div>
          {:else if section === 'playlists' && playlists.length < playlistTotal}
            <div
              style="display:flex; justify-content:center; padding:18px 0 4px;"
            >
              <button
                type="button"
                class="btn"
                onclick={() => onLoadMorePlaylists?.()}
                disabled={playlistsLoadingMore}
                >{playlistsLoadingMore
                  ? 'Loading…'
                  : 'Load More Playlists'}</button
              >
            </div>
          {/if}

          {#if section === 'playlists' && filteredUnavailablePlaylists.length > 0}
            <details
              open={collectionState === 'attention'}
              style="margin-top:18px; border-top:1px solid var(--divider); padding-top:12px;"
            >
              <summary
                style="cursor:pointer; color:var(--text-secondary); font-size:10px; font-weight:600;"
                >Unavailable Playlists · {filteredUnavailablePlaylists.length}</summary
              >
              <div
                style="display:grid; grid-template-columns:repeat(auto-fill,minmax(220px,1fr)); gap:8px; margin-top:9px;"
              >
                {#each filteredUnavailablePlaylists as playlist (playlist.id)}
                  <div class="inspector-card" style="margin:0; padding:10px;">
                    <strong style="font-size:10px;">{playlist.name}</strong>
                    <p
                      style="margin:3px 0 0; color:var(--text-tertiary); font-size:9px; line-height:1.45;"
                    >
                      {playlist.accessIssue ??
                        'Spotify does not expose this playlist to Refrain.'}
                    </p>
                  </div>
                {/each}
              </div>
            </details>
          {/if}
        </div>
      </div>
    {/if}
  {/if}
</div>
