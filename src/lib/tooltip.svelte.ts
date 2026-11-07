/**
 * One shared tooltip for the whole app. It is rendered once at the top level,
 * so it is never clipped by cards or covered by the footer or dialogs, and it
 * is clamped to the viewport. It only follows hover and keyboard focus; a click
 * never pins it open.
 */
class TooltipController {
  text = $state('');
  anchor = $state<HTMLElement | null>(null);

  show(anchor: HTMLElement, text: string) {
    if (!text) return;
    this.anchor = anchor;
    this.text = text;
  }

  hide() {
    this.anchor = null;
    this.text = '';
  }
}

export const tooltip = new TooltipController();

const GAP = 10;
const MARGIN = 8;

/** Places a box of size w×h next to `anchor`, preferring above, never outside the viewport. */
export function placeTooltip(anchor: DOMRect, w: number, h: number, vw: number, vh: number) {
  const x = Math.min(Math.max(MARGIN, anchor.left - 12), vw - w - MARGIN);
  let y = anchor.top - h - GAP;
  if (y < MARGIN) y = Math.min(anchor.bottom + GAP, vh - h - MARGIN);
  return { x: Math.max(MARGIN, x), y: Math.max(MARGIN, y) };
}
