/**
 * Attachment for modal dialogs (A1): moves focus into `node` (to `initial`,
 * else the first focusable element), keeps Tab and Shift+Tab inside it, and
 * gives focus back to whatever had it when the dialog closes.
 */
const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), summary, [tabindex]:not([tabindex="-1"])';

export function focusables(node: HTMLElement): HTMLElement[] {
  return [...node.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (el) => !el.closest("[inert], [hidden]") && el.getAttribute("aria-hidden") !== "true",
  );
}

export function focusTrap(initial?: () => HTMLElement | null | undefined) {
  return (node: HTMLElement) => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    (initial?.() ?? focusables(node)[0] ?? node).focus();

    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;
      const items = focusables(node);
      if (items.length === 0) {
        e.preventDefault();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      if (e.shiftKey && (active === first || !node.contains(active))) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && (active === last || !node.contains(active))) {
        e.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("keydown", onKey, true);
      if (previous?.isConnected) previous.focus();
    };
  };
}
