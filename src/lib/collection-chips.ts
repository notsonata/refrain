/** Number of whole labels that fit alongside a +N badge for the hidden labels. */
export function countFittingCollectionLabels(
  labelWidths: number[],
  overflowWidths: number[],
  availableWidth: number,
  gap = 4,
  countOnlyBelow = 0,
): number {
  if (availableWidth <= countOnlyBelow) return 0;

  const fullWidth =
    labelWidths.reduce((total, width) => total + width, 0) +
    Math.max(0, labelWidths.length - 1) * gap;
  if (fullWidth <= availableWidth) return labelWidths.length;

  for (let count = labelWidths.length - 1; count >= 0; count -= 1) {
    const hiddenCount = labelWidths.length - count;
    const overflowWidth = overflowWidths[hiddenCount - 1] ?? 30;
    const visibleWidth = labelWidths
      .slice(0, count)
      .reduce((total, width) => total + width, 0);
    if (
      visibleWidth + overflowWidth + (count > 0 ? count * gap : 0) <=
      availableWidth
    )
      return count;
  }

  return 0;
}
