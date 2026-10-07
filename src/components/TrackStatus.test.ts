import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import TrackStatus from './TrackStatus.svelte';

describe('TrackStatus', () => {
  it('renders Needs Local Copy with the warning color', () => {
    const { body } = render(TrackStatus, {
      props: { status: 'needs-local-copy' },
    });

    expect(body).toContain('state-dot warning');
    expect(body).toContain('Needs Local Copy');
  });

  it('keeps Spotify Only informational', () => {
    const { body } = render(TrackStatus, {
      props: { status: 'spotify-only' },
    });

    expect(body).toContain('state-dot info');
  });
});
