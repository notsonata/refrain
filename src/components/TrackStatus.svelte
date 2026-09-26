<script lang="ts">
  import { trackStatusLabel, type TrackStatus } from '../lib/track-status';

  export let status: TrackStatus;
</script>

<span class="state-text">
  {#if status === 'local-spotify'}
    <span class="state-dot success"></span>
  {:else if status === 'local-only'}
    <span class="state-dot warning"></span>
  {:else}
    <span class="state-dot info"></span>
  {/if}
  {trackStatusLabel(status)}
</span>
