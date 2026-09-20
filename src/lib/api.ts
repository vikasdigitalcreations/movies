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
}

export interface SystemInfo {
  version: string;
  downloadDir: string;
  logDir: string;
  screenshotDir: string;
  freeBytes?: number | null;
}

export const api = {
  home: (tab: string) => invoke<HomeRow[]>("home", { tab }),
  search: (query: string, page: number, filter: string) => invoke<Card[]>("search", { query, page, filter }),
  suggest: (query: string) => invoke<string[]>("suggest", { query }),
  details: (id: string) => invoke<Details>("details", { id }),
  // title/year let the backend fall back to the other source when MovieBox withholds a
  // title; leave them out and only MovieBox is consulted.
  streams: (id: string, season: number, episode: number, absIndex: number, title?: string, year?: string | null, preferred?: number) =>
    invoke<Stream[]>("streams", { id, season, episode, absIndex, title: title ?? null, year: year ?? null, preferred: preferred ?? null }),
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
