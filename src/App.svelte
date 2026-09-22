<script lang="ts">
  import { onMount } from 'svelte';
  import { getAppInfo, type AppInfo } from './lib/app-info';

  let appInfo: AppInfo | null = null;
  let backendError: string | null = null;

  onMount(() => {
    void loadAppInfo();
  });

  async function loadAppInfo() {
    try {
      appInfo = await getAppInfo();
      backendError = null;
    } catch (error) {
      backendError = error instanceof Error ? error.message : String(error);
    }
  }
</script>

<svelte:head><title>Refrain</title></svelte:head>

<main class="min-h-screen bg-slate-950 text-slate-100">
  <div class="mx-auto flex min-h-screen max-w-5xl flex-col px-8 py-10">
    <header class="border-b border-slate-800 pb-6">
      <p class="text-xs font-semibold uppercase tracking-[0.28em] text-slate-500">Local music sync</p>
      <div class="mt-2 flex items-end justify-between gap-6">
        <div>
          <h1 class="text-3xl font-semibold tracking-tight">Refrain</h1>
          <p class="mt-2 max-w-2xl text-sm leading-6 text-slate-400">
            Mirror Spotify playlists and Liked Songs into a durable local music library.
          </p>
        </div>
        <span class="rounded-full border border-slate-800 px-3 py-1 text-xs text-slate-400">v0.1 foundation</span>
      </div>
    </header>

    <section class="flex flex-1 items-center justify-center py-12">
      <div class="w-full max-w-xl rounded-xl border border-slate-800 bg-slate-900/60 p-6 shadow-2xl shadow-black/20">
        <div class="flex items-center justify-between gap-4">
          <div>
            <h2 class="text-base font-medium">Desktop runtime</h2>
            <p class="mt-1 text-sm text-slate-400">Typed frontend-to-Rust command smoke test.</p>
          </div>
          {#if appInfo}
            <span class="rounded-full bg-emerald-500/10 px-3 py-1 text-xs font-medium text-emerald-300">Connected</span>
          {:else if backendError}
            <span class="rounded-full bg-amber-500/10 px-3 py-1 text-xs font-medium text-amber-300">Unavailable</span>
          {:else}
            <span class="rounded-full bg-slate-800 px-3 py-1 text-xs font-medium text-slate-300">Checking</span>
          {/if}
        </div>

        {#if appInfo}
          <dl class="mt-6 grid gap-4 text-sm">
            <div class="grid grid-cols-[8rem_1fr] gap-4">
              <dt class="text-slate-500">Application</dt>
              <dd>{appInfo.name} {appInfo.version}</dd>
            </div>
            <div class="grid grid-cols-[8rem_1fr] gap-4">
              <dt class="text-slate-500">Data directory</dt>
              <dd class="min-w-0 break-all font-mono text-xs text-slate-300">{appInfo.dataDir}</dd>
            </div>
          </dl>
        {:else if backendError}
          <div class="mt-6 rounded-lg border border-amber-900/50 bg-amber-950/30 p-4 text-sm text-amber-200">
            The Tauri backend is not available. Run this interface through <code class="font-mono">npm run tauri dev</code>.
          </div>
        {:else}
          <div class="mt-6 h-16 animate-pulse rounded-lg bg-slate-800/70"></div>
        {/if}
      </div>
    </section>

    <footer class="border-t border-slate-800 pt-5 text-xs text-slate-600">
      Milestone 1 establishes the application foundation. Spotify synchronization is implemented in later v0.1 milestones.
    </footer>
  </div>
</main>
