import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { StagingItem } from '../lib/staging';
import StagingView from './StagingView.svelte';

const base: StagingItem = {
  libraryTrackId: 7,
  jobId: null,
  title: 'Tracked Song',
  artists: ['Artist'],
  album: 'Album',
  durationMs: 180_000,
  imageUrl: null,
  provider: null,
  providerJobId: null,
  stage: 'needsLocalCopy',
  jobStatus: null,
  attempt: 0,
  candidate: null,
  candidates: [],
  stagingPath: null,
  errorCode: null,
  errorMessage: null,
  bytesTransferred: null,
  totalBytes: null,
  createdAt: null,
  startedAt: null,
  finishedAt: null,
  updatedAt: null,
};

describe('StagingView', () => {
  it('renders the tracked-only acquisition queue without global action buttons', () => {
    const { body } = render(StagingView, {
      props: {
        items: [base],
        total: 1,
        selectedLibraryTrackId: 7,
      },
    });

    expect(body).toContain('Staging');
    expect(body).toContain('Needs Local Copy');
    expect(body).not.toContain('Start All');
    expect(body).not.toContain('Cancel Active');
    expect(body).not.toContain('Retry Failed');
    expect(body).not.toContain('Continue Selected');
    expect(body).toContain('Tracked Song');
    expect(body).toContain('Select Tracked Song');
  });

  it('renders real download progress when byte counts exist', () => {
    const { body } = render(StagingView, {
      props: {
        items: [
          {
            ...base,
            jobId: 11,
            stage: 'downloading',
            jobStatus: 'running',
            bytesTransferred: 50,
            totalBytes: 100,
          },
        ],
        total: 1,
        selectedLibraryTrackId: 7,
      },
    });

    expect(body).toContain('50%');
    expect(body).toContain('Downloading');
  });

  it('uses indeterminate progress when download byte counts are unavailable', () => {
    const { body } = render(StagingView, {
      props: {
        items: [
          {
            ...base,
            jobId: 11,
            stage: 'downloading',
            jobStatus: 'running',
          },
        ],
        total: 1,
        selectedLibraryTrackId: 7,
      },
    });

    expect(body).toMatch(/class="card-progress[^"]*indeterminate/);
    expect(body).not.toContain('width:20%');
  });

  it('renders manual candidate resolution actions', () => {
    const { body } = render(StagingView, {
      props: {
        items: [
          {
            ...base,
            jobId: 11,
            stage: 'needsResolution',
            jobStatus: 'failed',
            candidates: [
              {
                provider: 'monochrome',
                providerToken: 'candidate-a',
                source: 'listener123',
                fileName: 'Artist/Album/Tracked Song.flac',
                title: 'Tracked Song',
                artists: ['Artist'],
                album: 'Album',
                durationMs: 180_000,
                format: 'flac',
                sizeBytes: 12_000_000,
                confidence: 92,
              },
            ],
          },
        ],
        total: 1,
        selectedLibraryTrackId: 7,
      },
    });

    expect(body).toContain('Needs Resolution');
    expect(body).toContain('Add');
    expect(body).toContain('Reject');
    expect(body).toContain('Search again');
    expect(body).toContain('92%');
  });

  it('offers exclusion from tracking only through multi-select actions', () => {
    const props = {
      items: [
        {
          ...base,
          jobId: 11,
          stage: 'failed' as const,
          jobStatus: 'failed' as const,
          errorMessage: 'Provider failed.',
        },
      ],
      total: 1,
      selectedLibraryTrackId: 7,
    };

    const unselected = render(StagingView, { props });
    expect(unselected.body).not.toContain('Exclude from tracking');

    const selected = render(StagingView, {
      props: { ...props, selectedTrackIds: [7] },
    });
    expect(selected.body).toContain('staging-exclude-selected');
    expect(selected.body.match(/Exclude from tracking/g)).toHaveLength(1);
  });

  it('orders manual candidates by highest confidence', () => {
    const { body } = render(StagingView, {
      props: {
        items: [
          {
            ...base,
            jobId: 11,
            stage: 'needsResolution',
            jobStatus: 'failed',
            candidates: [
              {
                provider: 'monochrome',
                providerToken: 'candidate-low',
                source: null,
                fileName: 'Lower Match.flac',
                title: 'Lower Match',
                artists: ['Artist'],
                album: 'Album',
                durationMs: 180_000,
                format: 'flac',
                sizeBytes: null,
                confidence: 61,
              },
              {
                provider: 'monochrome',
                providerToken: 'candidate-high',
                source: null,
                fileName: 'Higher Match.flac',
                title: 'Higher Match',
                artists: ['Artist'],
                album: 'Album',
                durationMs: 180_000,
                format: 'flac',
                sizeBytes: null,
                confidence: 88,
              },
            ],
          },
        ],
        total: 1,
        selectedLibraryTrackId: 7,
      },
    });

    expect(body.indexOf('Higher Match')).toBeLessThan(
      body.indexOf('Lower Match'),
    );
    expect(body).toContain('88%');
    expect(body).toContain('61%');
  });

  it('groups manual candidates by provider without mixing provider priority', () => {
    const { body } = render(StagingView, {
      props: {
        items: [
          {
            ...base,
            jobId: 11,
            provider: 'sockseek',
            stage: 'needsResolution',
            jobStatus: 'failed',
            candidates: [
              {
                provider: 'monochrome',
                providerToken: 'mono-low',
                source: null,
                fileName: 'Mono Low.flac',
                title: 'Mono Low',
                artists: ['Artist'],
                album: 'Album',
                durationMs: 180_000,
                format: 'flac',
                sizeBytes: null,
                confidence: 70,
              },
              {
                provider: 'sockseek',
                providerToken: 'sock-high',
                source: null,
                fileName: 'Sock High.flac',
                title: 'Sock High',
                artists: ['Artist'],
                album: 'Album',
                durationMs: 180_000,
                format: 'flac',
                sizeBytes: null,
                confidence: 95,
              },
            ],
          },
        ],
        total: 1,
        selectedLibraryTrackId: 7,
      },
    });

    expect(body).toContain('Monochrome');
    expect(body).toContain('Sockseek');
    expect(body.indexOf('Monochrome')).toBeLessThan(body.indexOf('Sockseek'));
    expect(body.indexOf('Mono Low')).toBeLessThan(body.indexOf('Sock High'));
    expect(body).toContain('2 providers');
  });

  it('shows manual-resolution file quality on its own readable line', () => {
    const { body } = render(StagingView, {
      props: {
        items: [
          {
            ...base,
            jobId: 11,
            provider: 'sockseek',
            stage: 'needsResolution',
            jobStatus: 'failed',
            candidates: [
              {
                provider: 'sockseek',
                providerToken: 'lossless-candidate',
                source: 'listener123',
                fileName: 'Artist/Album/Tracked Song.flac',
                title: 'Tracked Song',
                artists: ['Artist'],
                album: 'Album',
                durationMs: 180_000,
                format: 'flac',
                sizeBytes: 25_000_000,
                bitrateKbps: 980,
                sampleRateHz: 96_000,
                bitDepth: 24,
                confidence: 85,
              },
            ],
          },
        ],
        total: 1,
        selectedLibraryTrackId: 7,
      },
    });

    expect(body).toContain('FLAC');
    expect(body).toContain('980 kbps');
    expect(body).toContain('96 kHz');
    expect(body).toContain('24-bit');
    expect(body).toContain('candidate-file-info');
    expect(body).toContain('FLAC · 96 kHz · 24-bit · 980 kbps');
    expect(body).toContain('>96 kHz · 24-bit</dd>');
  });

  it('shows the acquisition provider beneath the downloaded format', () => {
    const { body } = render(StagingView, {
      props: {
        items: [
          {
            ...base,
            jobId: 11,
            provider: 'antra',
            stage: 'downloaded',
            jobStatus: 'staged',
            candidate: {
              provider: 'antra',
              providerToken: 'antra-candidate',
              source: 'Tidal',
              fileName: 'Tracked Song.flac',
              title: 'Tracked Song',
              artists: ['Artist'],
              album: 'Album',
              durationMs: 180_000,
              format: 'flac',
              sizeBytes: null,
              sampleRateHz: 96_000,
              bitDepth: 24,
            },
          },
        ],
        total: 1,
        selectedLibraryTrackId: 7,
      },
    });

    expect(body).toContain('<small>FLAC</small>');
    expect(body).toContain('download-quality');
    expect(body).toContain('96 kHz · 24-bit');
    expect(body).toContain('download-provider');
    expect(body).toContain('>Antra</small>');
    expect(body).toContain('>Quality</dt>');
    expect(body).toContain('>96 kHz · 24-bit</dd>');
  });

  it('keeps manual resolution controls available while a batch action is busy', () => {
    const { body } = render(StagingView, {
      props: {
        items: [
          {
            ...base,
            jobId: 11,
            stage: 'needsResolution',
            jobStatus: 'failed',
            candidates: [
              {
                provider: 'monochrome',
                providerToken: 'candidate-a',
                source: null,
                fileName: 'Tracked Song.flac',
                title: 'Tracked Song',
                artists: ['Artist'],
                album: 'Album',
                durationMs: 180_000,
                format: 'flac',
                sizeBytes: null,
              },
            ],
          },
        ],
        total: 1,
        selectedLibraryTrackId: 7,
        busy: true,
      },
    });

    expect(body).not.toMatch(/class="btn btn-primary" disabled[^>]*>Download/);
    expect(body).not.toMatch(/class="btn" disabled[^>]*>Reject/);
    expect(body).not.toMatch(/class="btn" disabled[^>]*>Search Again/);
  });

  it('shows useful recording metadata for manual candidate decisions', () => {
    const { body } = render(StagingView, {
      props: {
        items: [
          {
            ...base,
            jobId: 11,
            stage: 'needsResolution',
            jobStatus: 'failed',
            candidates: [
              {
                provider: 'monochrome',
                providerToken: 'candidate-a',
                source: null,
                fileName: 'Lagi Na Lang.flac',
                title: 'Lagi Na Lang',
                artists: ['Sugarcane'],
                album: null,
                durationMs: 249_725,
                format: 'flac',
                sizeBytes: null,
                isrc: 'PHW012400262',
                recordingId: '212456142083723264',
                releaseId: '169623492306694144',
              },
            ],
          },
        ],
        total: 1,
        selectedLibraryTrackId: 7,
      },
    });

    expect(body).toContain('4:10');
    expect(body).toContain('ISRC PHW012400262');
    expect(body).toContain('Release 169623492306694144');
  });

  it('shows contextual actions only after tracks are selected', () => {
    const { body } = render(StagingView, {
      props: {
        items: [
          {
            ...base,
            jobId: 22,
            stage: 'failed',
            jobStatus: 'failed',
          },
        ],
        total: 1,
        selectedLibraryTrackId: 7,
        selectedTrackIds: [7],
      },
    });

    expect(body).toContain('1 selected');
    expect(body).toMatch(/Retry\s+1/);
    expect(body).toMatch(/type="checkbox"[^>]*checked/);
  });

  it('shows a direct retry action for the selected failed track', () => {
    const { body } = render(StagingView, {
      props: {
        items: [
          {
            ...base,
            jobId: 22,
            stage: 'failed',
            jobStatus: 'failed',
            errorCode: 'antraDownloadFailed',
            errorMessage: 'Antra returned HTTP 404 Not Found.',
          },
        ],
        total: 1,
        selectedLibraryTrackId: 7,
      },
    });

    expect(body).toContain('antraDownloadFailed');
    expect(body).toContain('inspector-retry');
    expect(body).toContain('class="btn inspector-retry');
    expect(body).not.toContain('btn-primary inspector-retry');
    expect(body).toMatch(/Retry/);
  });

  it('allows selected needs-resolution tracks to be retried', () => {
    const { body } = render(StagingView, {
      props: {
        items: [
          {
            ...base,
            jobId: 23,
            stage: 'needsResolution',
            jobStatus: 'failed',
          },
        ],
        total: 1,
        selectedLibraryTrackId: 7,
        selectedTrackIds: [7],
      },
    });

    expect(body).toMatch(/Retry\s+1/);
  });

  it('renders source artwork in the queue and right-side inspector', () => {
    const { body } = render(StagingView, {
      props: {
        items: [{ ...base, imageUrl: 'https://i.scdn.co/image/example' }],
        total: 1,
        selectedLibraryTrackId: 7,
      },
    });

    expect(body).toContain('https://i.scdn.co/image/example');
    expect(body.match(/Tracked Song artwork/g)?.length).toBe(2);
  });

  it('anchors active download progress in the bottom transfer bar', () => {
    const { body } = render(StagingView, {
      props: {
        items: [
          {
            ...base,
            jobId: 11,
            stage: 'downloading',
            jobStatus: 'running',
            bytesTransferred: 25,
            totalBytes: 100,
          },
        ],
        total: 1,
        selectedLibraryTrackId: 7,
      },
    });

    expect(body).toContain('Current download progress');
    expect(body).toContain('25%');
  });

  it('becomes a detailed sync page when nothing is waiting to download', () => {
    const { body } = render(StagingView, {
      props: {
        items: [],
        total: 0,
        syncBusy: true,
        syncRun: {
          id: 5,
          scope: 'spotify',
          trigger: 'manual',
          status: 'running',
          phase: 'verifyImports',
          startedAt: 1_700_000_000_000,
          finishedAt: null,
          sourceAdded: 2,
          sourceRemoved: 0,
          matched: 10,
          missing: 3,
          needsReview: 1,
          acquisitionFailed: 0,
          errorMessage: null,
        },
      },
    });

    expect(body).toContain('Verify and import');
    expect(body).toContain('Synchronization phases');
    expect(body).toContain('Matched');
    expect(body).toContain('10');
    expect(body).toContain('Current synchronization progress');
    expect(body).toContain('Download → verify → import.');
  });
});
