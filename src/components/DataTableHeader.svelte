<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from './Icon.svelte';
  import { tableGridWidth, type TableColumn } from '../lib/table-columns';

  export let columns: TableColumn[] = [];
  export let storageKey: string;
  export let sortId: string;
  export let sortDirection: 'asc' | 'desc' = 'asc';

  let draggedId: string | null = null;

  onMount(() => {
    const stored = localStorage.getItem(storageKey);
    if (!stored) return;

    try {
      const parsed = JSON.parse(stored) as {
        columns?: Array<{ id: string; width?: number }>;
        sortId?: string;
        sortDirection?: 'asc' | 'desc';
      };
      const byId = new Map(columns.map((column) => [column.id, column]));
      const restored = (parsed.columns ?? [])
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

      columns = [...restored, ...byId.values()];
      if (
        parsed.sortId &&
        columns.some((column) => column.id === parsed.sortId)
      ) {
        sortId = parsed.sortId;
      }
      if (parsed.sortDirection === 'asc' || parsed.sortDirection === 'desc') {
        sortDirection = parsed.sortDirection;
      }
    } catch {
      localStorage.removeItem(storageKey);
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
  }

  function setSort(column: TableColumn) {
    if (!column.sortable) return;
    if (sortId === column.id) {
      sortDirection = sortDirection === 'asc' ? 'desc' : 'asc';
    } else {
      sortId = column.id;
      sortDirection = 'asc';
    }
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
    columns = [...next, ...locked];
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

  $: gridTemplate = columns.map((column) => `${column.width}px`).join(' ');
  $: tableWidth = tableGridWidth(columns);
</script>

<div
  class="table-head column-table-grid"
  style={`grid-template-columns:${gridTemplate}; width:${tableWidth}px; min-width:${tableWidth}px;`}
>
  {#each columns as column (column.id)}
    <div
      class="table-column-header"
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
