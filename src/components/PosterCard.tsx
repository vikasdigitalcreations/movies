import { memo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Check, Film, Tv } from "lucide-react";
import type { Card, HistoryItem } from "../lib/api";
import { posterUrl } from "../lib/format";
import { useApp } from "../store/app";

export const PosterCard = memo(function PosterCard({
  card,
  history,
  width,
  navRow,
  navCol,
  subtitle,
  onOpen,
}: {
  card: Card;
  history?: HistoryItem;
  width?: number;
  navRow?: string;
  navCol?: number;
  subtitle?: string;
  onOpen?: () => void;
}) {
  const navigate = useNavigate();
  const openMenu = useApp((s) => s.openContextMenu);
  const [loaded, setLoaded] = useState(false);
  const [failed, setFailed] = useState(false);
  const pct = history && history.duration ? Math.min(100, (history.progress / history.duration) * 100) : 0;
  const open = onOpen ?? (() => navigate(`/title/${card.id}`, { state: { card } }));

  return (
    <div
      role="button"
      tabIndex={0}
      data-nav-row={navRow}
      data-nav-col={navCol}
      aria-label={`${card.title}${card.year ? ` (${card.year})` : ""}`}
      title={card.title}
      onClick={open}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          open();
        }
        if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
          e.preventDefault();
          const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
          openMenu({ x: r.left + r.width / 2, y: r.top + r.height / 2, card, history });
        }
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        openMenu({ x: e.clientX, y: e.clientY, card, history });
      }}
      className="group relative shrink-0 cursor-pointer outline-none"
      style={width ? { width } : undefined}
    >
      <div className="relative aspect-[2/3] overflow-hidden rounded-lg bg-panel2 ring-white/80 transition duration-200 group-hover:scale-[1.04] group-hover:ring-2 group-focus-visible:scale-[1.04] group-focus-visible:ring-4">
        {!loaded && !failed && <div className="skeleton absolute inset-0" />}
        {card.poster && !failed ? (
          <img
            src={posterUrl(card.poster)}
            alt=""
            loading="lazy"
            draggable={false}
            onLoad={() => setLoaded(true)}
            onError={() => setFailed(true)}
            className={`h-full w-full object-cover transition-opacity duration-300 ${loaded ? "opacity-100" : "opacity-0"}`}
          />
        ) : (
          failed || !card.poster ? (
            <div className="flex h-full w-full flex-col items-center justify-center gap-2 p-3 text-center text-white/50">
              {card.mediaType === "series" ? <Tv size={36} /> : <Film size={36} />}
              <span className="line-clamp-3 text-sm">{card.title}</span>
            </div>
          ) : null
        )}
        {history?.completed && (
          <div className="absolute right-2 top-2 flex h-7 w-7 items-center justify-center rounded-full bg-black/75 text-green-400" title="Watched">
            <Check size={16} strokeWidth={3} />
          </div>
        )}
        {card.mediaType === "series" && (
          <div className="absolute left-2 top-2 rounded bg-black/70 px-1.5 py-0.5 text-[11px] font-bold uppercase tracking-wide text-white/90">Series</div>
        )}
        {pct > 0 && !history?.completed && (
          <div className="absolute inset-x-0 bottom-0 h-1.5 bg-white/25">
            <div className="h-full bg-brand" style={{ width: `${pct}%` }} />
          </div>
        )}
      </div>
      <div className="mt-2 px-0.5">
        <div className="truncate text-sm font-semibold text-white/90">{card.title}</div>
        <div className="truncate text-xs text-white/50">{subtitle ?? card.year ?? ""}</div>
      </div>
    </div>
  );
});
