<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from './Icon.svelte';
  import {
    mergeSharedColumns,
    migrateLegacyColumns,
    restoredColumnWidth,
    tableGridTemplate,
    type TableColumn,
  } from '../lib/table-columns';

  export let columns: TableColumn[] = [];
  export let storageKey: string;
  export let legacyStorageKey: string | undefined = undefined;
  export let legacyStorageKeys: readonly string[] = [];
  export let resetLegacyWidthIds: readonly string[] = [];
  export let sortId: string;
  export let sortDirection: 'asc' | 'desc' = 'asc';
  export let compact = false;
  export let sharedStorageKey: string | undefined = undefined;
  export let sharedColumnIds: readonly string[] = [];
  export let defaultSortId: string | undefined = undefined;
  export let defaultSortLabel = 'Order';

  let draggedId: string | null = null;

  type SavedTableState = {
    columns?: Array<{ id: string; width?: number }>;
    sortId?: string;
    sortDirection?: 'asc' | 'desc';
  };

  function readState(key: string): SavedTableState | null {
    const stored = localStorage.getItem(key);
    if (!stored) return null;

    try {
      return JSON.parse(stored) as SavedTableState;
    } catch {
      localStorage.removeItem(key);
      return null;
    }
  }

  function keepActionsLast(nextColumns: TableColumn[]): TableColumn[] {
    const order = nextColumns.find((column) => column.id === 'order');
    const actions = nextColumns.find((column) => column.id === 'actions');
    return [
      ...(order ? [order] : []),
      ...nextColumns.filter(
        (column) => column.id !== 'order' && column.id !== 'actions',
      ),
      ...(actions ? [actions] : []),
    ];
  }

  function restoreColumns(
    savedColumns: SavedTableState['columns'],
    allowedIds: ReadonlySet<string> | undefined = undefined,
  ) {
    if (!savedColumns?.length) return;

    const byId = new Map(
      columns
        .filter((column) => !allowedIds || allowedIds.has(column.id))
        .map((column) => [column.id, column]),
    );
    const restored = savedColumns
      .map((saved) => {
        const current = byId.get(saved.id);
        if (!current) return null;
        byId.delete(saved.id);
        return {
          ...current,
          width: restoredColumnWidth(current, saved.width),
        };
      })
      .filter((column): column is TableColumn => column !== null);

    if (allowedIds) {
      const shared = [...restored, ...byId.values()];
      let sharedIndex = 0;
      columns = keepActionsLast(
        columns.map((column) =>
          allowedIds.has(column.id) ? shared[sharedIndex++] : column,
        ),
      );
      return;
    }

    const untouched = columns.filter(
      (column) =>
        !restored.some((restoredColumn) => restoredColumn.id === column.id),
    );
    columns = keepActionsLast([...restored, ...untouched]);
  }

  onMount(() => {
    const tableState = readState(storageKey);
    const legacyState = tableState
      ? null
      : ([...legacyStorageKeys, ...(legacyStorageKey ? [legacyStorageKey] : [])]
          .map(readState)
          .find((state) => state !== null) ?? null);
    const activeState = tableState ?? legacyState;
    if (activeState) {
      restoreColumns(
        legacyState
          ? migrateLegacyColumns(activeState.columns, resetLegacyWidthIds)
          : activeState.columns,
      );
      if (
        activeState.sortId &&
        (columns.some((column) => column.id === activeState.sortId) ||
          activeState.sortId === defaultSortId)
      ) {
        sortId = activeState.sortId;
      }
      if (
        activeState.sortDirection === 'asc' ||
        activeState.sortDirection === 'desc'
      ) {
        sortDirection = activeState.sortDirection;
      }
    }

    if (sharedStorageKey && sharedColumnIds.length > 0) {
      const sharedState = readState(sharedStorageKey);
      if (sharedState) {
        restoreColumns(sharedState.columns, new Set(sharedColumnIds));
      }
    }
    if (legacyState) persist();
  });

  function persist() {
    localStorage.setItem(
      storageKey,
      JSON.stringify({
        columns: columns.map(({ id, width }) => ({ id, width })),
        sortId,
        sortDirection,
      }),
    );

    if (sharedStorageKey && sharedColumnIds.length > 0) {
      const sharedIds = new Set(sharedColumnIds);
      const previousShared = readState(sharedStorageKey);
      localStorage.setItem(
        sharedStorageKey,
        JSON.stringify({
          columns: mergeSharedColumns(
            columns,
            previousShared?.columns ?? [],
            sharedIds,
          ),
        }),
      );
    }
  }

  function setSort(column: TableColumn) {
    if (!column.sortable) return;
    if (sortId === column.id) {
      if (sortDirection === 'asc') {
        sortDirection = 'desc';
      } else if (defaultSortId && defaultSortId !== column.id) {
        sortId = defaultSortId;
        sortDirection = 'asc';
      } else {
        sortDirection = 'asc';
      }
    } else {
      sortId = column.id;
      sortDirection = 'asc';
    }
    persist();
  }

  function selectSort(event: Event) {
    sortId = (event.currentTarget as HTMLSelectElement).value;
    sortDirection = 'asc';
    persist();
  }

  function toggleSortDirection() {
    sortDirection = sortDirection === 'asc' ? 'desc' : 'asc';
    persist();
  }

  function startDrag(event: DragEvent, column: TableColumn) {
    if (column.draggable === false) {
      event.preventDefault();
      return;
    }
    draggedId = column.id;
    event.dataTransfer?.setData('text/plain', column.id);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
  }

  function dropColumn(event: DragEvent, target: TableColumn) {
    event.preventDefault();
    if (!draggedId || target.draggable === false || draggedId === target.id) {
      draggedId = null;
      return;
    }

    const movable = columns.filter((column) => column.draggable !== false);
    const locked = columns.filter((column) => column.draggable === false);
    const from = movable.findIndex((column) => column.id === draggedId);
    const to = movable.findIndex((column) => column.id === target.id);
    if (from < 0 || to < 0) return;

    const next = [...movable];
    const [moved] = next.splice(from, 1);
    next.splice(to, 0, moved);
    columns = keepActionsLast([...next, ...locked]);
    draggedId = null;
    persist();
  }

  function startResize(event: PointerEvent, column: TableColumn) {
    event.preventDefault();
    event.stopPropagation();
    const startX = event.clientX;
    const startWidth = column.width;

    const move = (moveEvent: PointerEvent) => {
      const width = Math.max(
        column.minWidth,
        startWidth + moveEvent.clientX - startX,
      );
      columns = columns.map((item) =>
        item.id === column.id ? { ...item, width } : item,
      );
    };
    const stop = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', stop);
      persist();
    };

    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', stop, { once: true });
  }

  $: gridTemplate = tableGridTemplate(columns);
  $: sortableColumns = columns.filter((column) => column.sortable);
  $: compactSortOptions = [
    ...(defaultSortId &&
    !sortableColumns.some((column) => column.id === defaultSortId)
      ? [{ id: defaultSortId, label: defaultSortLabel }]
      : []),
    ...sortableColumns.map(({ id, label }) => ({ id, label })),
  ];
</script>

{#if compact}
  <div class="compact-table-sortbar">
    <span>Sort</span>
    <select aria-label="Sort rows" value={sortId} onchange={selectSort}>
      {#each compactSortOptions as option (option.id)}
        <option value={option.id}>{option.label}</option>
      {/each}
    </select>
    <button
      type="button"
      class="row-more compact-sort-direction"
      onclick={toggleSortDirection}
      aria-label={sortDirection === 'asc'
        ? 'Sort ascending'
        : 'Sort descending'}
      title={sortDirection === 'asc' ? 'Ascending' : 'Descending'}
    >
      <Icon
        name={sortDirection === 'asc' ? 'sort-asc' : 'sort-desc'}
        size={14}
      />
    </button>
  </div>
{:else}
  <div
    class="table-head column-table-grid"
    style={`grid-template-columns:${gridTemplate};`}
  >
    {#each columns as column (column.id)}
      <div
        class="table-column-header"
        data-column-id={column.id}
        class:dragging={draggedId === column.id}
        class:center={column.align === 'center'}
        class:right={column.align === 'right'}
        role="columnheader"
        tabindex="0"
        draggable={column.draggable !== false}
        ondragstart={(event) => startDrag(event, column)}
        ondragend={() => (draggedId = null)}
        ondragover={(event) => {
          if (column.draggable !== false) event.preventDefault();
        }}
        ondrop={(event) => dropColumn(event, column)}
      >
        {#if column.sortable}
          <button
            type="button"
            class="table-sort-button"
            class:active={sortId === column.id}
            onclick={() => setSort(column)}
            title={`Sort by ${column.label}`}
          >
            <span>{column.label}</span>
            {#if sortId === column.id}
              <Icon
                name={sortDirection === 'asc' ? 'sort-asc' : 'sort-desc'}
                size={12}
              />
            {/if}
          </button>
        {:else}
          <span class="table-column-label">{column.label}</span>
        {/if}

        {#if column.draggable !== false}
          <span class="column-drag-indicator" aria-hidden="true">
            <Icon name="grip" size={11} />
          </span>
        {/if}
        {#if column.resizable !== false}
          <span
            class="column-resize-handle"
            role="separator"
            aria-orientation="vertical"
            aria-label={`Resize ${column.label || 'column'}`}
            onpointerdown={(event) => startResize(event, column)}
          ></span>
        {/if}
      </div>
    {/each}
  </div>
{/if}
