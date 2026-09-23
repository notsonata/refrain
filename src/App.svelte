<script lang="ts">
  import { onMount } from 'svelte';
  import { getAppInfo, type AppInfo } from './lib/app-info';
  import {
    connectSpotify,
    disconnectSpotify,
    getSpotifyAuthStatus,
    spotifyErrorMessage,
    type SpotifyAuthStatus,
  } from './lib/spotify';

  const fallbackRedirectUri = 'http://127.0.0.1/callback';

  let appInfo: AppInfo | null = null;
  let authStatus: SpotifyAuthStatus | null = null;
  let clientId = '';
  let backendError: string | null = null;
  let authError: string | null = null;
  let authBusy = false;

  onMount(() => {
    void initialize();
  });

  async function initialize() {
    try {
      const [info, status] = await Promise.all([
        getAppInfo(),
        getSpotifyAuthStatus(),
      ]);
      appInfo = info;
      authStatus = status;
      clientId = status.clientId ?? '';
      backendError = null;
    } catch (error) {
      backendError = spotifyErrorMessage(error);
    }
  }

  async function connect() {
    authBusy = true;
    authError = null;
    try {
      authStatus = await connectSpotify(clientId);
      clientId = authStatus.clientId ?? clientId.trim();
    } catch (error) {
      authError = spotifyErrorMessage(error);
    } finally {
      authBusy = false;
    }
  }

  async function disconnect() {
    authBusy = true;
    authError = null;
    try {
      authStatus = await disconnectSpotify();
    } catch (error) {
      authError = spotifyErrorMessage(error);
    } finally {
      authBusy = false;
    }
  }
</script>

<svelte:head>
  <title>Refrain</title>
</svelte:head>

<main class="min-h-screen bg-slate-950 text-slate-100">
  <div class="mx-auto flex min-h-screen max-w-5xl flex-col px-8 py-10">
    <header class="border-b border-slate-800 pb-6">
      <p class="text-xs uppercase tracking-widest text-slate-500">
        Spotify source setup
      </p>
      <div class="mt-2 flex items-end justify-between gap-6">
        <div>
          <h1 class="text-3xl font-semibold">Refrain</h1>
          <p class="mt-2 max-w-2xl text-sm text-slate-400">
            Connect your own Spotify developer application. Refrain uses PKCE
            and never needs your Spotify client secret.
          </p>
        </div>
        <span class="rounded-full border border-slate-800 px-3 py-1 text-xs">
          v0.1 authentication
        </span>
      </div>
    </header>

    <section class="grid flex-1 gap-6 py-10 lg:grid-cols-[minmax(0,1fr)_18rem]">
      <div class="rounded-xl border border-slate-800 p-6">
        <div class="flex items-start justify-between gap-4">
          <div>
            <h2 class="text-lg font-medium">Spotify connection</h2>
            <p class="mt-1 text-sm text-slate-400">
              The Client ID is saved locally. Refresh credentials are stored in
              your operating system credential store.
            </p>
          </div>
          {#if authStatus?.connected}
            <span class="rounded-full bg-emerald-950 px-3 py-1 text-xs text-emerald-300">
              Connected
            </span>
          {:else}
            <span class="rounded-full bg-slate-900 px-3 py-1 text-xs text-slate-300">
              Not connected
            </span>
          {/if}
        </div>

        <div class="mt-6 grid gap-2">
          <label for="spotify-client-id" class="text-sm font-medium">
            Spotify Client ID
          </label>
          <input
            id="spotify-client-id"
            bind:value={clientId}
            disabled={authBusy || authStatus?.connected}
            autocomplete="off"
            spellcheck="false"
            placeholder="Paste your Spotify Client ID"
            class="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2.5 font-mono text-sm outline-none transition focus:border-slate-500 disabled:cursor-not-allowed disabled:opacity-60"
          />
        </div>

        <div class="mt-5 rounded-lg border border-slate-800 bg-slate-900/60 p-4">
          <p class="text-xs font-medium uppercase tracking-wider text-slate-500">
            Spotify redirect URI
          </p>
          <code class="mt-2 block break-all text-sm text-slate-200">
            {authStatus?.registeredRedirectUri ?? fallbackRedirectUri}
          </code>
          <p class="mt-2 text-xs leading-5 text-slate-400">
            Add this exact URI to your Spotify developer application. Refrain
            listens on a dynamic port on 127.0.0.1 during sign-in.
          </p>
        </div>

        {#if authError}
          <div
            class="mt-5 rounded-lg border border-amber-900 bg-amber-950/30 p-4 text-sm text-amber-200"
          >
            {authError}
          </div>
        {/if}

        <div class="mt-6 flex items-center gap-3">
          {#if authStatus?.connected}
            <button
              type="button"
              onclick={disconnect}
              disabled={authBusy}
              class="rounded-lg border border-slate-700 px-4 py-2 text-sm font-medium transition hover:bg-slate-900 disabled:cursor-not-allowed disabled:opacity-60"
            >
              {authBusy ? 'Disconnecting…' : 'Disconnect'}
            </button>
          {:else}
            <button
              type="button"
              onclick={connect}
              disabled={authBusy || !clientId.trim()}
              class="rounded-lg bg-slate-100 px-4 py-2 text-sm font-medium text-slate-950 transition hover:bg-white disabled:cursor-not-allowed disabled:opacity-50"
            >
              {authBusy ? 'Waiting for Spotify…' : 'Connect Spotify'}
            </button>
          {/if}
          {#if authBusy}
            <span class="text-xs text-slate-500">
              Complete authorization in your browser.
            </span>
          {/if}
        </div>
      </div>

      <aside class="rounded-xl border border-slate-800 p-5 text-sm">
        <h2 class="font-medium">Runtime</h2>
        {#if appInfo}
          <dl class="mt-4 grid gap-4">
            <div>
              <dt class="text-xs uppercase tracking-wider text-slate-600">
                Application
              </dt>
              <dd class="mt-1">{appInfo.name} {appInfo.version}</dd>
            </div>
            <div>
              <dt class="text-xs uppercase tracking-wider text-slate-600">
                Data directory
              </dt>
              <dd class="mt-1 break-all font-mono text-xs text-slate-400">
                {appInfo.dataDir}
              </dd>
            </div>
          </dl>
        {:else if backendError}
          <p class="mt-4 text-sm text-amber-300">{backendError}</p>
        {:else}
          <div class="mt-4 h-16 animate-pulse rounded-lg bg-slate-800"></div>
        {/if}
      </aside>
    </section>

    <footer class="border-t border-slate-800 pt-5 text-xs text-slate-600">
      Spotify library retrieval and manual source refresh arrive in the next
      v0.1 milestone.
    </footer>
  </div>
</main>
