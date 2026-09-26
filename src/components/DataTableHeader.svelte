<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from './Icon.svelte';
  import { tableGridTemplate, type TableColumn } from '../lib/table-columns';

  export let columns: TableColumn[] = [];
  export let storageKey: string;
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
          width: Math.max(
            current.minWidth,
            typeof saved.width === 'number' ? saved.width : current.width,
          ),
        };
      })
      .filter((column): column is TableColumn => column !== null);

    if (allowedIds) {
      const shared = [...restored, ...byId.values()];
      const specific = columns.filter((column) => !allowedIds.has(column.id));
      columns = keepActionsLast([...shared, ...specific]);
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
    if (tableState) {
      restoreColumns(tableState.columns);
      if (
        tableState.sortId &&
        (columns.some((column) => column.id === tableState.sortId) ||
          tableState.sortId === defaultSortId)
      ) {
        sortId = tableState.sortId;
      }
      if (
        tableState.sortDirection === 'asc' ||
        tableState.sortDirection === 'desc'
      ) {
        sortDirection = tableState.sortDirection;
      }
    }

    if (sharedStorageKey && sharedColumnIds.length > 0) {
      const sharedState = readState(sharedStorageKey);
      if (sharedState) {
        restoreColumns(sharedState.columns, new Set(sharedColumnIds));
      }
    }
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
      localStorage.setItem(
        sharedStorageKey,
        JSON.stringify({
          columns: columns
            .filter((column) => sharedIds.has(column.id))
            .map(({ id, width }) => ({ id, width })),
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
        <span
          class="column-resize-handle"
          role="separator"
          aria-orientation="vertical"
          aria-label={`Resize ${column.label || 'column'}`}
          onpointerdown={(event) => startResize(event, column)}
        ></span>
      </div>
    {/each}
  </div>
{/if}
