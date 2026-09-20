import { create } from "zustand";
import { useNavigate } from "react-router-dom";
import type { PlayRef, Season, Stream } from "../lib/api";

export interface Session {
  ref: PlayRef;
  seasons: Season[];
  dubIds: string[];
  episodeTitle?: string | null;
  /** null = ask to resume if there is saved progress */
  startAt: number | null;
  /** play a downloaded file instead of streaming */
  localPath?: string;
  /** play this exact stream, for sources that resolve outside the usual providers */
  directStream?: Stream;
  qualityOverride?: number | null;
}

interface PlayerStore {
  session: Session | null;
  start: (s: Session) => void;
  clear: () => void;
}

export const usePlayer = create<PlayerStore>((set) => ({
  session: null,
  start: (s) => set({ session: s }),
  clear: () => set({ session: null }),
}));

export function usePlayNow() {
  const navigate = useNavigate();
  const start = usePlayer((s) => s.start);
  return (s: Session) => {
    start(s);
    navigate("/play");
  };
}

export function neighbor(seasons: Season[], season: number, episode: number, dir: 1 | -1): { season: number; episode: number; title?: string | null } | null {
  const flat: { season: number; episode: number; title?: string | null }[] = [];
  for (const s of [...seasons].sort((a, b) => a.number - b.number)) {
    for (const e of [...s.episodes].sort((a, b) => a.number - b.number)) flat.push({ season: s.number, episode: e.number, title: e.title });
  }
  const i = flat.findIndex((x) => x.season === season && x.episode === episode);
  if (i < 0) return null;
  return flat[i + dir] ?? null;
}
