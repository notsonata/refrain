<script lang="ts">
  import { onDestroy } from 'svelte';
  import type { OverflowMenuItem } from '../lib/menu';
  import Icon from './Icon.svelte';

  export let items: OverflowMenuItem[] = [];
  export let ariaLabel = 'More actions';

  let open = false;
  let trigger: HTMLButtonElement;
  let menu: HTMLDivElement;
  let top = 0;
  let left = 0;
  let listening = false;

  const menuWidth = 196;

  function onPointerDown(event: PointerEvent) {
    const target = event.target;
    if (!(target instanceof Node)) return;
    if (!trigger.contains(target) && !menu?.contains(target)) close();
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === 'Escape') close();
  }

  function onViewportChange() {
    close();
  }

  function startListening() {
    if (listening) return;
    listening = true;
    document.addEventListener('pointerdown', onPointerDown);
    document.addEventListener('keydown', onKeyDown);
    document.addEventListener('scroll', onViewportChange, true);
    window.addEventListener('resize', onViewportChange);
  }

  function stopListening() {
    if (!listening) return;
    listening = false;
    document.removeEventListener('pointerdown', onPointerDown);
    document.removeEventListener('keydown', onKeyDown);
    document.removeEventListener('scroll', onViewportChange, true);
    window.removeEventListener('resize', onViewportChange);
  }

  function close() {
    if (!open && !listening) return;
    open = false;
    stopListening();
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
    startListening();
    positionMenu();
  }

  function run(item: OverflowMenuItem) {
    if (item.disabled) return;
    close();
    try {
      const result = item.action();
      void Promise.resolve(result).catch((error) => {
        console.error('Overflow menu action failed', error);
      });
    } catch (error) {
      console.error('Overflow menu action failed', error);
    }
  }

  function runFromPointer(event: PointerEvent, item: OverflowMenuItem) {
    if (event.button !== 0 || item.disabled) return;
    event.preventDefault();
    run(item);
  }

  onDestroy(stopListening);
</script>

{#if items.length > 0}
  <span class="overflow-menu-shell">
    <button
      bind:this={trigger}
      type="button"
      class="row-more"
      aria-label={ariaLabel}
      title={ariaLabel}
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
        {#each items as item (item.label)}
          {#if item.separatorBefore}
            <div class="overflow-menu-separator" role="separator"></div>
          {/if}
          <button
            type="button"
            class="overflow-menu-item"
            role="menuitem"
            disabled={item.disabled}
            onpointerdown={(event) => runFromPointer(event, item)}
            onclick={(event) => {
              if (event.detail === 0) run(item);
            }}
          >
            <Icon name={item.icon} size={14} />
            <span>{item.label}</span>
          </button>
        {/each}
      </div>
    {/if}
  </span>
{/if}
