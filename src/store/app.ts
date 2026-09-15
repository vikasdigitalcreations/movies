import { create } from "zustand";
import { api, type DownloadTask, type Settings, type HistoryItem, type Card } from "../lib/api";

export interface Toast {
  id: number;
  kind: "info" | "success" | "error";
  text: string;
  action?: { label: string; run: () => void };
}

export interface ContextMenuState {
  x: number;
  y: number;
  card: Card;
  history?: HistoryItem;
}

interface AppStore {
  settings: Settings | null;
  downloads: DownloadTask[];
  history: HistoryItem[];
  favorites: Card[];
  toasts: Toast[];
  online: boolean;
  tourOpen: boolean;
  contextMenu: ContextMenuState | null;
  loadSettings: () => Promise<void>;
  saveSettings: (patch: Partial<Settings>) => Promise<void>;
  refreshDownloads: () => Promise<void>;
  upsertDownload: (t: DownloadTask) => void;
  removeDownload: (id: string) => void;
  refreshHistory: () => Promise<void>;
  refreshFavorites: () => Promise<void>;
  toast: (text: string, kind?: Toast["kind"], action?: Toast["action"]) => void;
  dismissToast: (id: number) => void;
  setOnline: (v: boolean) => void;
  setTourOpen: (v: boolean) => void;
  openContextMenu: (m: ContextMenuState | null) => void;
}

let toastSeq = 1;

export const useApp = create<AppStore>((set, get) => ({
  settings: null,
  downloads: [],
  history: [],
  favorites: [],
  toasts: [],
  online: true,
  tourOpen: false,
  contextMenu: null,
  loadSettings: async () => {
    const s = await api.settingsGet();
    set({ settings: s });
  },
  saveSettings: async (patch) => {
    const cur = get().settings;
    if (!cur) return;
    const next = { ...cur, ...patch };
    set({ settings: next });
    const saved = await api.settingsSet(next);
    set({ settings: saved });
  },
  refreshDownloads: async () => set({ downloads: await api.downloadList() }),
  upsertDownload: (t) =>
    set((st) => {
      const i = st.downloads.findIndex((d) => d.id === t.id);
      if (i < 0) return { downloads: [...st.downloads, t] };
      const copy = st.downloads.slice();
      copy[i] = t;
      return { downloads: copy };
    }),
  removeDownload: (id) => set((st) => ({ downloads: st.downloads.filter((d) => d.id !== id) })),
  refreshHistory: async () => set({ history: await api.historyList() }),
  refreshFavorites: async () => set({ favorites: await api.favoritesList() }),
  toast: (text, kind = "info", action) => {
    const id = toastSeq++;
    set((st) => ({ toasts: [...st.toasts.slice(-3), { id, kind, text, action }] }));
    setTimeout(() => get().dismissToast(id), action ? 8000 : 4500);
  },
  dismissToast: (id) => set((st) => ({ toasts: st.toasts.filter((t) => t.id !== id) })),
  setOnline: (v) => set({ online: v }),
  setTourOpen: (v) => set({ tourOpen: v }),
  openContextMenu: (m) => set({ contextMenu: m }),
}));
