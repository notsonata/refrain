import { describe, expect, it } from 'vitest';
import { countFittingCollectionLabels } from './collection-chips';

describe('collection chip overflow', () => {
  it('replaces one oversized playlist title with +1', () => {
    expect(countFittingCollectionLabels([230], [34], 120)).toBe(0);
    expect(countFittingCollectionLabels([100], [34], 120)).toBe(1);
  });

  it('preserves fitting names and counts every hidden playlist', () => {
    expect(countFittingCollectionLabels([48, 54, 90], [34, 34, 34], 98)).toBe(
      1,
    );
    expect(countFittingCollectionLabels([48, 54, 90], [34, 34, 34], 190)).toBe(
      2,
    );
    expect(countFittingCollectionLabels([48, 54, 90], [34, 34, 34], 210)).toBe(
      3,
    );
  });

  it('recomputes the count as the column narrows', () => {
    const widths = [110, 80];
    expect(countFittingCollectionLabels(widths, [34, 34], 200)).toBe(2);
    expect(countFittingCollectionLabels(widths, [34, 34], 160)).toBe(1);
    expect(countFittingCollectionLabels(widths, [34, 34], 80)).toBe(0);
    expect(countFittingCollectionLabels(widths, [34, 34], 48)).toBe(0);
  });

  it('collapses even short labels at the narrowest playlist width', () => {
    expect(countFittingCollectionLabels([24], [34], 48, 4, 64)).toBe(0);
    expect(countFittingCollectionLabels([24], [34], 80, 4, 64)).toBe(1);
  });
});
