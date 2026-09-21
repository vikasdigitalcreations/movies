import { invoke } from "@tauri-apps/api/core";

export type MediaType = "movie" | "series";

export interface Card {
  id: string;
  title: string;
  year?: string | null;
  poster?: string | null;
  mediaType: MediaType;
}

export interface HomeRow {
  title: string;
  kind: "hero" | "row";
  items: Card[];
}

export interface Episode {
  number: number;
  title?: string | null;
}
export interface Season {
  number: number;
  episodes: Episode[];
}
export interface Dub {
  subjectId: string;
  label: string;
}
export interface Details {
  id: string;
  title: string;
  mediaType: MediaType;
  year?: string | null;
  description?: string | null;
  imdbRating?: string | null;
  director?: string | null;
  stars?: string | null;
  poster?: string | null;
  duration?: string | null;
  genres: string[];
  seasons: Season[];
  dubs: Dub[];
  isFavorite: boolean;
}

export type AdultSource = "eporner" | "redgifs";

export interface AdultPlayback {
  /** "stream" plays in our player; "embed" has to open the site's own player. */
  kind: "stream" | "embed";
  stream?: Stream | null;
  embedUrl?: string | null;
  source: string;
}

export interface AddonCatalog {
  manifestUrl: string;
  addonName: string;
  kind: string;
  id: string;
  name: string;
  locked: boolean;
}

export interface Addon {
  manifestUrl: string;
  name: string;
  version?: string | null;
  description?: string | null;
  enabled: boolean;
  providesCatalog: boolean;
  providesMeta: boolean;
  providesStream: boolean;
  /** Cinemeta: needed to match titles to addons, so it can't be removed. */
  core: boolean;
}

export interface Stream {
  label: string;
  height: number;
  multi: boolean;
  size?: number | null;
  codec?: string | null;
  url: string;
  headers: [string, string][];
  resourceId?: string | null;
  downloadable: boolean;
  source: string;
}

export interface Subtitle {
  name: string;
  url: string;
}

export interface HistoryItem {
  id: string;
  title: string;
  poster?: string | null;
  mediaType: MediaType;
  year: string;
  season: number;
  episode: number;
  progress: number;
  duration?: number | null;
  completed: boolean;
  updated: number;
}

export interface PlayRef {
  id: string;
  title: string;
  poster?: string | null;
  mediaType: MediaType;
  year?: string | null;
  season: number;
  episode: number;
}

export type DownloadStatus = "queued" | "downloading" | "paused" | "done" | "failed";
export interface DownloadTask {
  id: string;
  subjectId: string;
  title: string;
  year?: string | null;
  poster?: string | null;
  mediaType: MediaType;
  season: number;
  episode: number;
  absIndex: number;
  episodeTitle?: string | null;
  height: number;
  url: string;
  headers: [string, string][];
  subtitleUrl?: string | null;
  subtitleLang?: string | null;
  path: string;
  status: DownloadStatus;
  downloaded: number;
  total?: number | null;
  speed: number;
  error?: string | null;
  added: number;
}

export interface NewDownload {
  subjectId: string;
  title: string;
  year?: string | null;
  poster?: string | null;
  mediaType: MediaType;
  season: number;
  episode: number;
  absIndex: number;
  episodeTitle?: string | null;
  height: number;
  url: string;
  headers: [string, string][];
  size?: number | null;
  subtitleUrl?: string | null;
  subtitleLang?: string | null;
}

export interface Settings {
  preferredQuality: number;
  subtitleLanguage: string;
  autoplayNext: boolean;
  rememberSpeed: boolean;
  seekStep: number;
  downloadDir?: string | null;
  simultaneousDownloads: number;
  subtitleSize: number;
  subtitleBackground: boolean;
  volume: number;
  lastSpeed: number;
  nightMode: boolean;
  uiZoom: number;
  tourDone: boolean;
  youtubeSource: boolean;
}

export interface SystemInfo {
  version: string;
  downloadDir: string;
  logDir: string;
  screenshotDir: string;
  freeBytes?: number | null;
}

// ---- stream prefetch -------------------------------------------------------
// Links stay good for far longer than this (a signed MovieBox link was still fetchable
// eleven minutes after it was issued); the short life is so a stale answer is never reused.
const PREFETCH_TTL_MS = 120_000;
const prefetched = new Map<string, { at: number; p: Promise<Stream[]> }>();
const streamsKey = (id: string, season: number, episode: number, preferred?: number) => `${id}|${season}|${episode}|${preferred ?? 0}`;
const askStreams = (
  id: string,
  season: number,
  episode: number,
  absIndex: number,
  title: string | undefined,
  year: string | null | undefined,
  preferred: number | undefined,
  primaryOnly: boolean,
) => invoke<Stream[]>("streams", { id, season, episode, absIndex, title: title ?? null, year: year ?? null, preferred: preferred ?? null, primaryOnly });

export const api = {
  home: (tab: string) => invoke<HomeRow[]>("home", { tab }),
  search: (query: string, page: number, filter: string) => invoke<Card[]>("search", { query, page, filter }),
  suggest: (query: string) => invoke<string[]>("suggest", { query }),
  details: (id: string) => invoke<Details>("details", { id }),
  // title/year let the backend fall back to the other source when MovieBox withholds a
  // title; leave them out and only MovieBox is consulted.
  // --- PIN (guards the Adults section and any addon you lock)
  pinIsSet: () => invoke<boolean>("pin_is_set"),
  pinSet: (pin: string, current?: string) => invoke<void>("pin_set", { pin, current: current ?? null }),
  pinVerify: (pin: string) => invoke<boolean>("pin_verify", { pin }),
  pinClear: (current: string) => invoke<void>("pin_clear", { current }),
  addonSetLocked: (url: string, locked: boolean) => invoke<void>("addon_set_locked", { url, locked }),

  // --- Adults
  adultIsEnabled: () => invoke<boolean>("adult_is_enabled"),
  adultSetEnabled: (enabled: boolean) => invoke<void>("adult_set_enabled", { enabled }),
  adultSearch: (source: AdultSource, query: string, page: number, unlocked: boolean) =>
    invoke<Card[]>("adult_search", { source, query, page, unlocked }),
  adultPlayback: (id: string, unlocked: boolean) => invoke<AdultPlayback>("adult_playback", { id, unlocked }),

  // --- Addon catalogues
  addonCatalogs: (unlocked: boolean) => invoke<AddonCatalog[]>("addon_catalogs", { unlocked }),
  addonCatalogItems: (manifestUrl: string, kind: string, id: string, skip: number, unlocked: boolean) =>
    invoke<Card[]>("addon_catalog_items", { manifestUrl, kind, id, skip, unlocked }),
  addonsList: () => invoke<Addon[]>("addons_list"),
  addonsAdd: (url: string) => invoke<Addon>("addons_add", { url }),
  addonsRemove: (url: string) => invoke<void>("addons_remove", { url }),
  addonsToggle: (url: string, enabled: boolean) => invoke<void>("addons_toggle", { url, enabled }),
  addonStreams: (title: string, year: string | null | undefined, isSeries: boolean, season: number, episode: number) =>
    invoke<Stream[]>("addon_streams", { title, year: year ?? null, isSeries, season, episode }),
  /**
   * Looks for a title's streams before anyone presses Play, so Play has them already.
   * Asks MovieBox alone: merely opening a page must not set the heavier backup scrapers
   * going. If MovieBox has nothing the entry is dropped and `streams` does the full search.
   */
  prefetchStreams: (id: string, season: number, episode: number, absIndex: number, title?: string, year?: string | null, preferred?: number) => {
    const k = streamsKey(id, season, episode, preferred);
    const hit = prefetched.get(k);
    if (hit && Date.now() - hit.at < PREFETCH_TTL_MS) return;
    const entry = { at: Date.now(), p: askStreams(id, season, episode, absIndex, title, year, preferred, true) };
    prefetched.set(k, entry);
    entry.p.catch(() => {
      if (prefetched.get(k) === entry) prefetched.delete(k);
    });
    while (prefetched.size > 8) prefetched.delete(prefetched.keys().next().value as string);
  },
  /** Streams for a title. Uses (and uses up) a fresh prefetch if there is one. */
  streams: (id: string, season: number, episode: number, absIndex: number, title?: string, year?: string | null, preferred?: number) => {
    const k = streamsKey(id, season, episode, preferred);
    const hit = prefetched.get(k);
    prefetched.delete(k);
    if (hit && Date.now() - hit.at < PREFETCH_TTL_MS) {
      // A prefetch that came back empty or failed only means MovieBox has nothing; the full search still can.
      return hit.p.catch(() => askStreams(id, season, episode, absIndex, title, year, preferred, false));
    }
    return askStreams(id, season, episode, absIndex, title, year, preferred, false);
  },
  subtitles: (id: string, resourceId: string, dubIds: string[], season: number, episode: number) =>
    invoke<Subtitle[]>("subtitles", { id, resourceId, dubIds, season, episode }),
  fetchSubtitle: (url: string) => invoke<string>("fetch_subtitle", { url }),
  alternateSource: (title: string, year: string | null | undefined, season: number, episode: number, preferred: number) =>
    invoke<Stream>("alternate_source", { title, year: year ?? null, season, episode, preferred }),

  historyList: () => invoke<HistoryItem[]>("history_list"),
  historyGet: (id: string, season: number, episode: number) => invoke<HistoryItem | null>("history_get", { id, season, episode }),
  historyWatched: (id: string, episodes: [number, number][]) => invoke<boolean[]>("history_watched", { id, episodes }),
  historyStart: (item: PlayRef, position: number) => invoke<void>("history_start", { item, position: Math.floor(position) }),
  historyProgress: (item: PlayRef, position: number, duration: number | null) =>
    invoke<boolean>("history_progress", { item, position: Math.floor(position), duration: duration ? Math.floor(duration) : null }),
  historyMarkWatched: (item: PlayRef) => invoke<void>("history_mark_watched", { item }),
  historyRemove: (id: string) => invoke<void>("history_remove", { id }),
  historyClear: () => invoke<void>("history_clear"),

  favoritesList: () => invoke<Card[]>("favorites_list"),
  favoritesToggle: (card: Card) => invoke<boolean>("favorites_toggle", { card }),
  favoritesClear: () => invoke<void>("favorites_clear"),

  downloadList: () => invoke<DownloadTask[]>("download_list"),
  downloadAdd: (task: NewDownload) => invoke<DownloadTask>("download_add", { task }),
  downloadPause: (id: string) => invoke<void>("download_pause", { id }),
  downloadResume: (id: string) => invoke<void>("download_resume", { id }),
  downloadRemove: (id: string, deleteFile: boolean) => invoke<void>("download_remove", { id, deleteFile }),

  settingsGet: () => invoke<Settings>("settings_get"),
  settingsSet: (value: Settings) => invoke<Settings>("settings_set", { value }),
  systemInfo: () => invoke<SystemInfo>("system_info"),
  freeSpaceFor: (path: string) => invoke<number | null>("free_space_for", { path }),
  openFolder: (path: string) => invoke<void>("open_folder", { path }),
  openLogs: () => invoke<void>("open_logs"),
  clearCache: () => invoke<void>("clear_cache"),
  keepAwake: (display: boolean, system: boolean) => invoke<void>("keep_awake", { display, system }),
  checkOnline: () => invoke<boolean>("check_online"),
};

export function errText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return "Something went wrong. Please try again.";
}
