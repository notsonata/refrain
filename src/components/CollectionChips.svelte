<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';

  export let labels: string[] = [];

  let host: HTMLDivElement;
  let measureHost: HTMLDivElement;
  let visibleCount = labels.length;
  let observer: ResizeObserver | null = null;
  let frame = 0;
  let mounted = false;

  $: hiddenLabels = labels.slice(visibleCount);

  function scheduleMeasure() {
    if (!mounted) return;
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => void measure());
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
    const gap = 4;

    const fullWidth =
      widths.reduce((total, width) => total + width, 0) +
      Math.max(0, widths.length - 1) * gap;

    if (fullWidth <= available) {
      visibleCount = labels.length;
      return;
    }

    for (let count = labels.length - 1; count >= 0; count -= 1) {
      const hiddenCount = labels.length - count;
      const overflowWidth =
        overflowNodes[hiddenCount - 1]?.getBoundingClientRect().width ?? 30;
      const visibleWidth = widths
        .slice(0, count)
        .reduce((total, width) => total + width, 0);
      const gaps = count > 0 ? count * gap : 0;

      if (visibleWidth + overflowWidth + gaps <= available) {
        visibleCount = count;
        return;
      }
    }

    visibleCount = 0;
  }

  onMount(() => {
    mounted = true;
    observer = new ResizeObserver(scheduleMeasure);
    observer.observe(host);
    scheduleMeasure();
  });

  onDestroy(() => {
    observer?.disconnect();
    if (mounted) cancelAnimationFrame(frame);
    mounted = false;
  });

  function scheduleMeasureForLabels(currentLabels: string[]) {
    if (currentLabels === labels) scheduleMeasure();
  }

  $: scheduleMeasureForLabels(labels);
</script>

<div class="collection-chip-group" bind:this={host}>
  {#each labels.slice(0, visibleCount) as label, index (`${index}:${label}`)}
    <span class="chip">{label}</span>
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
</div>
