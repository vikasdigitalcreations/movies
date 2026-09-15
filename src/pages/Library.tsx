import { useEffect, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { Bookmark, History } from "lucide-react";
import { PosterCard } from "../components/PosterCard";
import { Button, EmptyState, PageHeader } from "../components/ui";
import { useApp } from "../store/app";

export function MyListPage() {
  const favorites = useApp((s) => s.favorites);
  const refresh = useApp((s) => s.refreshFavorites);
  const history = useApp((s) => s.history);
  const navigate = useNavigate();
  const histMap = useMemo(() => new Map(history.map((h) => [h.id, h])), [history]);
  useEffect(() => {
    refresh();
  }, [refresh]);

  return (
    <div className="h-full overflow-y-auto px-8 pb-10 pt-8">
      <PageHeader title="My List" subtitle="Titles you saved to watch later." />
      {favorites.length === 0 ? (
        <EmptyState icon={<Bookmark size={36} />} title="Your list is empty" text='Open any title and press "My List" to save it here.' action={<Button variant="primary" onClick={() => navigate("/")}>Browse titles</Button>} />
      ) : (
        <div className="fade-in grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-x-4 gap-y-6">
          {favorites.map((c, i) => (
            <PosterCard key={c.id} card={c} history={histMap.get(c.id)} navRow="grid" navCol={i} />
          ))}
        </div>
      )}
    </div>
  );
}

export function ContinuePage() {
  const history = useApp((s) => s.history);
  const refresh = useApp((s) => s.refreshHistory);
  const navigate = useNavigate();
  useEffect(() => {
    refresh();
  }, [refresh]);
  const inProgress = history.filter((h) => !h.completed);
  const watched = history.filter((h) => h.completed);

  const grid = (items: typeof history, row: string) => (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-x-4 gap-y-6">
      {items.map((h, i) => (
        <PosterCard
          key={h.id}
          navRow={row}
          navCol={i}
          card={{ id: h.id, title: h.title, poster: h.poster, mediaType: h.mediaType, year: h.year }}
          history={h}
          subtitle={h.mediaType === "series" ? `Season ${h.season} · Episode ${h.episode}` : h.year}
        />
      ))}
    </div>
  );

  return (
    <div className="h-full overflow-y-auto px-8 pb-10 pt-8">
      <PageHeader title="Continue Watching" subtitle="Pick up where you left off. Right-click a poster to remove it." />
      {history.length === 0 ? (
        <EmptyState icon={<History size={36} />} title="Nothing here yet" text="Titles you start watching will appear here." action={<Button variant="primary" onClick={() => navigate("/")}>Browse titles</Button>} />
      ) : (
        <div className="fade-in">
          {inProgress.length > 0 && grid(inProgress, "grid")}
          {watched.length > 0 && (
            <>
              <h2 className="mb-4 mt-10 text-xl font-bold text-white/80">Watched</h2>
              {grid(watched, "grid2")}
            </>
          )}
        </div>
      )}
    </div>
  );
}
