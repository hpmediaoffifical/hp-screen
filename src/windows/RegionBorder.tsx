/** Thickness of the border window outside the region, in physical pixels (BORDER_PX in Rust). */
const BORDER_PX = 6;

/**
 * A white dashed line with a soft dark edge, just outside the recorded region, so it shows on
 * light and dark screens. The window is excluded from capture and lets the mouse through.
 */
export default function RegionBorder() {
  const band = BORDER_PX / devicePixelRatio;
  const line = Math.max(1, band / 3);
  return (
    <div
      className="region-border"
      style={{
        border: `${line}px dashed rgba(255, 255, 255, 0.95)`,
        margin: (band - line) / 2,
        inset: 0,
        boxShadow: `0 0 0 ${line / 2}px rgba(0, 0, 0, 0.45), inset 0 0 0 ${line / 2}px rgba(0, 0, 0, 0.45)`,
      }}
    />
  );
}
