<script module lang="ts">
  const resizeCallbacks = new Map<Element, () => void>();
  let sharedResizeObserver: ResizeObserver | null = null;

  function observeResize(element: Element, callback: () => void): () => void {
    if (typeof ResizeObserver === 'undefined') return () => {};
    if (!sharedResizeObserver) {
      sharedResizeObserver = new ResizeObserver((entries) => {
        for (const entry of entries) resizeCallbacks.get(entry.target)?.();
      });
    }
    resizeCallbacks.set(element, callback);
    sharedResizeObserver.observe(element);

    return () => {
      resizeCallbacks.delete(element);
      sharedResizeObserver?.unobserve(element);
      if (resizeCallbacks.size === 0) {
        sharedResizeObserver?.disconnect();
        sharedResizeObserver = null;
      }
    };
  }
</script>

<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import { countFittingCollectionLabels } from '../lib/collection-chips';

  export let labels: string[] = [];
  export let countOnlyBelow = 0;

  let host: HTMLDivElement;
  let measureHost: HTMLDivElement;
  let visibleCount = labels.length;
  let stopObservingResize: (() => void) | null = null;
  let frame = 0;
  let mounted = false;

  $: hiddenLabels = labels.slice(visibleCount);

  function scheduleMeasure() {
    if (!mounted) return;
    if (labels.length === 0) {
      if (frame) cancelAnimationFrame(frame);
      frame = 0;
      visibleCount = 0;
      return;
    }
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => void measure());
  }

  function syncObserver() {
    if (!mounted || !host) return;
    const shouldObserve = labels.length > 0;
    if (shouldObserve && !stopObservingResize) {
      stopObservingResize = observeResize(host, scheduleMeasure);
    } else if (!shouldObserve && stopObservingResize) {
      stopObservingResize();
      stopObservingResize = null;
    }
  }

  async function measure() {
    await tick();
    if (!host || !measureHost || labels.length === 0) {
      visibleCount = labels.length;
      return;
    }

    const available = host.clientWidth;
    if (available <= 0) return;

    const labelNodes = Array.from(
      measureHost.querySelectorAll<HTMLElement>('[data-measure-label]'),
    );
    const overflowNodes = Array.from(
      measureHost.querySelectorAll<HTMLElement>('[data-measure-overflow]'),
    );
    const widths = labelNodes.map((node) => node.getBoundingClientRect().width);
    const overflowWidths = overflowNodes.map(
      (node) => node.getBoundingClientRect().width,
    );
    visibleCount = countFittingCollectionLabels(
      widths,
      overflowWidths,
      available,
      4,
      countOnlyBelow,
    );
  }

  onMount(() => {
    mounted = true;
    syncObserver();
    scheduleMeasure();
  });

  onDestroy(() => {
    stopObservingResize?.();
    if (mounted) cancelAnimationFrame(frame);
    mounted = false;
  });

  function scheduleMeasureForLabels(currentLabels: string[]) {
    if (currentLabels !== labels || !mounted) return;
    syncObserver();
    scheduleMeasure();
  }

  $: scheduleMeasureForLabels(labels);
</script>

<div class="collection-chip-group" bind:this={host}>
  {#each labels.slice(0, visibleCount) as label, index (`${index}:${label}`)}
    <span class="chip collection-label-chip">{label}</span>
  {/each}
  {#if hiddenLabels.length > 0}
    <button
      type="button"
      class="chip collection-overflow-chip"
      data-tooltip={hiddenLabels.join('\n')}
      aria-label={`${hiddenLabels.length} more collections: ${hiddenLabels.join(', ')}`}
    >
      +{hiddenLabels.length}
    </button>
  {/if}

  {#if labels.length > 0}
    <div
      class="collection-chip-measure"
      bind:this={measureHost}
      aria-hidden="true"
    >
      {#each labels as label, index (`label:${index}:${label}`)}
        <span class="chip" data-measure-label>{label}</span>
      {/each}
      {#each labels as label, index (`overflow:${index}:${label}`)}
        <span class="chip" data-measure-overflow>+{index + 1}</span>
      {/each}
    </div>
  {/if}
</div>
