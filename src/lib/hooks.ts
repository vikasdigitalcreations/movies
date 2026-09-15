import { useEffect, useLayoutEffect, useRef } from "react";

const positions = new Map<string, number>();

/** Remembers the scroll position of a scroll container per key (e.g. per page). */
export function useScrollMemory(key: string, ready: boolean) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (ready && ref.current) ref.current.scrollTop = positions.get(key) ?? 0;
  }, [key, ready]);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const on = () => positions.set(key, el.scrollTop);
    el.addEventListener("scroll", on, { passive: true });
    return () => el.removeEventListener("scroll", on);
  }, [key, ready]);
  return ref;
}

export function isTyping(target: EventTarget | null) {
  const el = target as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || el.isContentEditable;
}

/** Arrow-key focus movement between elements marked with data-nav-row. */
export function moveFocus(key: string): boolean {
  const all = Array.from(document.querySelectorAll<HTMLElement>("[data-nav-row]")).filter((el) => el.offsetParent !== null);
  if (all.length === 0) return false;
  const active = document.activeElement as HTMLElement | null;
  if (!active || !active.dataset.navRow) {
    if (key === "ArrowDown" || key === "ArrowRight") {
      all[0].focus();
      all[0].scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
      return true;
    }
    return false;
  }
  const r = active.getBoundingClientRect();
  const cx = r.left + r.width / 2;
  const cy = r.top + r.height / 2;
  let best: HTMLElement | null = null;
  let bestScore = Infinity;
  for (const el of all) {
    if (el === active) continue;
    const b = el.getBoundingClientRect();
    const x = b.left + b.width / 2;
    const y = b.top + b.height / 2;
    const dx = x - cx;
    const dy = y - cy;
    let primary: number;
    let secondary: number;
    if (key === "ArrowRight") {
      if (el.dataset.navRow !== active.dataset.navRow || dx <= 4) continue;
      primary = dx;
      secondary = Math.abs(dy);
    } else if (key === "ArrowLeft") {
      if (el.dataset.navRow !== active.dataset.navRow || dx >= -4) continue;
      primary = -dx;
      secondary = Math.abs(dy);
    } else if (key === "ArrowDown") {
      if (dy <= r.height / 3) continue;
      primary = dy;
      secondary = Math.abs(dx);
    } else {
      if (dy >= -r.height / 3) continue;
      primary = -dy;
      secondary = Math.abs(dx);
    }
    const score = primary + secondary * 2.5;
    if (score < bestScore) {
      bestScore = score;
      best = el;
    }
  }
  if (!best) return false;
  best.focus({ preventScroll: true });
  best.scrollIntoView({ block: key === "ArrowUp" || key === "ArrowDown" ? "center" : "nearest", inline: "nearest", behavior: "smooth" });
  return true;
}
