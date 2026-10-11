import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Radio, RefreshCw, Search as SearchIcon, X } from "lucide-react";
import { api, errText, type LiveChannel, type LiveGuide } from "../lib/api";
import { Button, EmptyState, ErrorState, PageHeader, Spinner } from "../components/ui";
import { useScrollMemory } from "../lib/hooks";
import { usePlayNow } from "../store/player";
import { useApp } from "../store/app";

/** How many tiles are drawn at once; more are added as the page is scrolled. */
const PAGE = 120;
const ALL = "All";

/**
 * Kept outside the page on purpose. Opening a channel leaves this page for the player,
 * which unmounts it; without this, coming back would lose the list, the filters and how
 * far down the person had scrolled.
 */
const kept: { guide: LiveGuide | null; category: string; language: string; query: string; shown: number } = {
  guide: null,
  category: ALL,
  language: ALL,
  query: "",
  shown: PAGE,
};

function Tile({ ch, onOpen }: { ch: LiveChannel; onOpen: () => void }) {
  const [failed, setFailed] = useState(false);
  return (
    <button onClick={onOpen} data-nav-row="grid" title={ch.name} className="group text-left outline-none">
      <div className="relative flex aspect-video items-center justify-center overflow-hidden rounded-xl bg-panel2 p-5 ring-white/80 transition duration-200 group-hover:scale-[1.03] group-hover:ring-2 group-focus-visible:scale-[1.03] group-focus-visible:ring-4">
        {ch.logo && !failed ? (
          <img src={ch.logo} alt="" loading="lazy" referrerPolicy="no-referrer" draggable={false} onError={() => setFailed(true)} className="max-h-full max-w-full object-contain" />
        ) : (
          <span className="line-clamp-2 text-center text-lg font-bold text-white/55">{ch.name}</span>
        )}
        {ch.height >= 720 && <span className="absolute right-1.5 top-1.5 rounded bg-black/70 px-1.5 py-0.5 text-[10px] font-bold tracking-wide text-white/90">HD</span>}
      </div>
      <div className="mt-2 truncate px-0.5 text-sm font-semibold text-white/90">{ch.name}</div>
      <div className="truncate px-0.5 text-xs text-white/50">{[ch.category, ch.language].filter(Boolean).join(" · ")}</div>
    </button>
  );
}

export function LiveTVPage() {
  const [guide, setGuide] = useState<LiveGuide | null>(kept.guide);
  const [err, setErr] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [category, setCategory] = useState(kept.category);
  const [language, setLanguage] = useState(kept.language);
  const [query, setQuery] = useState(kept.query);
  const [shown, setShown] = useState(kept.shown);
  const playNow = usePlayNow();
  const toast = useApp((s) => s.toast);
  const scrollRef = useScrollMemory("live", guide !== null);
  const moreRef = useRef<HTMLDivElement>(null);

  kept.guide = guide;
  kept.category = category;
  kept.language = language;
  kept.query = query;
  kept.shown = shown;

  const load = useCallback(async () => {
    setErr(null);
    try {
      setGuide(await api.liveChannels());
    } catch (e) {
      setErr(errText(e));
    }
  }, []);

  // The backend answers from memory after the first time, so asking again on every visit
  // is cheap and picks up a list that has since been refreshed.
  useEffect(() => {
    load();
  }, [load]);

  const refresh = async () => {
    setRefreshing(true);
    try {
      const g = await api.liveChannels(true);
      setGuide(g);
      setErr(null);
      toast(g.stale ? "Couldn't download a newer channel list. Showing the saved one." : "Channel list updated.", g.stale ? "error" : "success");
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setRefreshing(false);
    }
  };

  const filtered = useMemo(() => {
    if (!guide) return [];
    const q = query.trim().toLowerCase();
    return guide.channels.filter((c) => (category === ALL || c.category === category) && (language === ALL || c.language === language) && (!q || c.name.toLowerCase().includes(q)));
  }, [guide, category, language, query]);

  // A filter that no longer exists after a refresh must not leave the page empty.
  useEffect(() => {
    if (!guide) return;
    if (category !== ALL && !guide.categories.includes(category)) setCategory(ALL);
    if (language !== ALL && !guide.languages.includes(language)) setLanguage(ALL);
  }, [guide, category, language]);

  const pick = (set: (v: string) => void) => (v: string) => {
    set(v);
    setShown(PAGE);
    if (scrollRef.current) scrollRef.current.scrollTop = 0;
  };

  useEffect(() => {
    const el = moreRef.current;
    if (!el) return;
    const io = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && setShown((n) => n + PAGE), { root: scrollRef.current, rootMargin: "600px" });
    io.observe(el);
    return () => io.disconnect();
    // Re-armed after each batch, so a tall window keeps filling until the sentinel is off screen.
  }, [scrollRef, shown, filtered.length]);

  const open = (c: LiveChannel) =>
    playNow({
      ref: { id: `live:${c.id}`, title: c.name, year: null, poster: c.logo ?? null, mediaType: "movie", season: 0, episode: 0 },
      seasons: [],
      dubIds: [],
      startAt: 0,
      directStream: c.streams[0],
      alternates: c.streams.slice(1),
      live: true,
    });

  const chip = (label: string) => (
    <button
      key={label}
      onClick={() => pick(setCategory)(label)}
      className={`rounded-xl px-3 py-1.5 text-sm font-semibold transition ${category === label ? "bg-brand text-white" : "bg-white/8 text-white/60 hover:bg-white/14"}`}
    >
      {label}
    </button>
  );

  const filtering = category !== ALL || language !== ALL || query.trim() !== "";

  return (
    <div ref={scrollRef} className="h-full overflow-y-auto px-8 pb-10 pt-8">
      <PageHeader
        title="LiveTV"
        subtitle={guide ? `${guide.channels.length.toLocaleString()} free live channels from public lists. Some will be off air at any moment.` : "Free live channels from public lists."}
        right={
          <Button onClick={refresh} disabled={refreshing || !guide} title="Download the newest channel list">
            {refreshing ? <Spinner size={16} /> : <RefreshCw size={16} />} Refresh
          </Button>
        }
      />

      {guide && (
        <>
          <div className="mb-3 flex flex-wrap items-center gap-2">{[ALL, ...guide.categories].map(chip)}</div>
          <div className="mb-5 flex flex-wrap items-center gap-3">
            <select
              value={language}
              onChange={(e) => pick(setLanguage)(e.target.value)}
              aria-label="Language"
              className="rounded-xl border border-white/10 bg-panel2 px-3 py-1.5 text-sm text-white outline-none focus:border-white/40"
            >
              <option value={ALL}>All languages</option>
              {guide.languages.map((l) => (
                <option key={l} value={l}>
                  {l}
                </option>
              ))}
            </select>
            <div className="flex items-center gap-2 rounded-xl border border-white/10 bg-black/30 px-3 py-1.5">
              <SearchIcon size={15} className="text-white/35" />
              <input value={query} onChange={(e) => pick(setQuery)(e.target.value)} placeholder="Find a channel…" className="w-56 bg-transparent text-sm outline-none placeholder:text-white/30" />
              {query && (
                <button onClick={() => pick(setQuery)("")} className="text-white/40 hover:text-white" title="Clear">
                  <X size={14} />
                </button>
              )}
            </div>
            <span className="ml-auto text-sm text-white/40">
              {filtering ? `${filtered.length.toLocaleString()} of ${guide.channels.length.toLocaleString()} channels` : ""}
              {guide.stale ? `${filtering ? " · " : ""}Couldn't refresh the list, showing the saved copy` : ""}
            </span>
          </div>
        </>
      )}

      {err && !guide ? (
        <ErrorState text={err} onRetry={load} />
      ) : !guide ? (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(170px,1fr))] gap-x-4 gap-y-5">
          {Array.from({ length: 24 }).map((_, i) => (
            <div key={i} className="skeleton aspect-video rounded-xl" />
          ))}
        </div>
      ) : filtered.length === 0 ? (
        <EmptyState
          icon={<Radio size={30} />}
          title="No channels match"
          text="Try a different word, category or language."
          action={
            <Button
              onClick={() => {
                setCategory(ALL);
                setLanguage(ALL);
                setQuery("");
                setShown(PAGE);
              }}
            >
              Show all channels
            </Button>
          }
        />
      ) : (
        <>
          <div className="grid grid-cols-[repeat(auto-fill,minmax(170px,1fr))] gap-x-4 gap-y-5">
            {filtered.slice(0, shown).map((c) => (
              <Tile key={c.id} ch={c} onOpen={() => open(c)} />
            ))}
          </div>
          {shown < filtered.length && <div ref={moreRef} className="h-10" />}
        </>
      )}
    </div>
  );
}
