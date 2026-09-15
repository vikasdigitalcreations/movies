export function fmtTime(sec: number | null | undefined): string {
  if (sec == null || !isFinite(sec) || sec < 0) return "0:00";
  const s = Math.floor(sec);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  const pad = (n: number) => n.toString().padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(r)}` : `${m}:${pad(r)}`;
}

export function fmtBytes(b: number | null | undefined): string {
  if (!b || b <= 0) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  let v = b;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v >= 100 || i < 2 ? 0 : 1)} ${units[i]}`;
}

export function fmtSpeed(bps: number): string {
  if (!bps || bps <= 0) return "";
  return `${fmtBytes(bps)}/s`;
}

export function fmtEta(remaining: number, bps: number): string {
  if (!bps || bps <= 0 || remaining <= 0) return "";
  const s = remaining / bps;
  if (s < 60) return "less than a minute left";
  if (s < 3600) return `${Math.round(s / 60)} min left`;
  const h = Math.floor(s / 3600);
  const m = Math.round((s % 3600) / 60);
  return `${h} h ${m} min left`;
}

export function epLabel(season: number, episode: number): string {
  return `S${season.toString().padStart(2, "0")}E${episode.toString().padStart(2, "0")}`;
}

/** 0-based index of an episode across all seasons (used for resource paging). */
export function absIndex(seasons: { number: number; episodes: unknown[] }[], season: number, episode: number): number {
  let n = 0;
  for (const s of seasons) {
    if (s.number < season) n += Math.max(1, s.episodes.length);
  }
  return n + Math.max(0, episode - 1);
}

export function posterUrl(url?: string | null, width = 342): string | undefined {
  if (!url) return undefined;
  if (url.includes("pbcdn.aoneroom.com") && !url.includes("?")) return `${url}?x-oss-process=image/resize,w_${width}`;
  return url;
}

export function clamp(v: number, lo: number, hi: number) {
  return Math.min(hi, Math.max(lo, v));
}
