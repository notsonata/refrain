<script lang="ts">
  import { onMount } from 'svelte';
  import type { OverflowMenuItem } from '../lib/menu';
  import Icon from './Icon.svelte';

  export let items: OverflowMenuItem[] = [];
  export let ariaLabel = 'More actions';

  let open = false;
  let trigger: HTMLButtonElement;
  let menu: HTMLDivElement;
  let top = 0;
  let left = 0;

  const menuWidth = 196;

  function close() {
    open = false;
  }

  function positionMenu() {
    const rect = trigger.getBoundingClientRect();
    left = Math.max(
      8,
      Math.min(window.innerWidth - menuWidth - 8, rect.right - menuWidth),
    );
    top = rect.bottom + 4;

    requestAnimationFrame(() => {
      if (!menu) return;
      const menuRect = menu.getBoundingClientRect();
      if (menuRect.bottom > window.innerHeight - 8) {
        top = Math.max(8, rect.top - menuRect.height - 4);
      }
    });
  }

  function toggle() {
    if (open) {
      close();
      return;
    }
    open = true;
    positionMenu();
  }

  function run(item: OverflowMenuItem) {
    if (item.disabled) return;
    close();
    void Promise.resolve(item.action()).catch((error) => {
      console.error('Overflow menu action failed', error);
    });
  }

  onMount(() => {
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (!trigger.contains(target) && !menu?.contains(target)) close();
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') close();
    };
    const onViewportChange = () => close();

    document.addEventListener('pointerdown', onPointerDown);
    document.addEventListener('keydown', onKeyDown);
    document.addEventListener('scroll', onViewportChange, true);
    window.addEventListener('resize', onViewportChange);

    return () => {
      document.removeEventListener('pointerdown', onPointerDown);
      document.removeEventListener('keydown', onKeyDown);
      document.removeEventListener('scroll', onViewportChange, true);
      window.removeEventListener('resize', onViewportChange);
    };
  });
</script>

{#if items.length > 0}
  <span class="overflow-menu-shell">
    <button
      bind:this={trigger}
      type="button"
      class="row-more"
      aria-label={ariaLabel}
      aria-haspopup="menu"
      aria-expanded={open}
      onclick={toggle}
    >
      <Icon name="more" size={16} />
    </button>
    {#if open}
      <div
        bind:this={menu}
        class="overflow-menu"
        role="menu"
        style={`top:${top}px; left:${left}px;`}
      >
        {#each items as item}
          <button
            type="button"
            class="overflow-menu-item"
            role="menuitem"
            disabled={item.disabled}
            onclick={() => run(item)}
          >
            <Icon name={item.icon} size={14} />
            <span>{item.label}</span>
          </button>
        {/each}
      </div>
    {/if}
  </span>
{/if}
