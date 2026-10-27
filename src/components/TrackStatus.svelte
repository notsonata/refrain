<script lang="ts">
  import { trackStatusLabel, type TrackStatus } from '../lib/track-status';

  export let status: TrackStatus;
</script>

<span class="state-text" title={trackStatusLabel(status)}>
  {#if status === 'local-spotify'}
    <span class="state-dot success"></span>
  {:else if status === 'local-only' || status === 'needs-local-copy'}
    <span class="state-dot warning"></span>
  {:else}
    <span class="state-dot info"></span>
  {/if}
  <span class="state-label">{trackStatusLabel(status)}</span>
</span>
