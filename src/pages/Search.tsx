import { useEffect, useMemo, useRef, useState } from "react";
import { Search as SearchIcon, X, SearchX } from "lucide-react";
import { api, errText, type Card } from "../lib/api";
import { PosterCard } from "../components/PosterCard";
import { EmptyState, ErrorState, SkeletonGrid, Spinner } from "../components/ui";
import { useApp } from "../store/app";

const memory = { query: "", filter: "all", results: [] as Card[], page: 1, done: false };

export function SearchPage() {
  const [query, setQuery] = useState(memory.query);
  const [submitted, setSubmitted] = useState(memory.query);
  const [filter, setFilter] = useState(memory.filter);
  const [results, setResults] = useState<Card[]>(memory.results);
  const [page, setPage] = useState(memory.page);
  const [done, setDone] = useState(memory.done);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const [showSug, setShowSug] = useState(false);
  const [sugIndex, setSugIndex] = useState(-1);
  const inputRef = useRef<HTMLInputElement>(null);
  const sentinel = useRef<HTMLDivElement>(null);
  const history = useApp((s) => s.history);
  const histMap = useMemo(() => new Map(history.map((h) => [h.id, h])), [history]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    Object.assign(memory, { query: submitted, filter, results, page, done });
  }, [submitted, filter, results, page, done]);

  // live suggestions
  useEffect(() => {
    if (query.trim().length < 2 || query === submitted) {
      setSuggestions([]);
      return;
    }
    const t = setTimeout(() => {
      api.suggest(query).then((s) => {
        setSuggestions(s);
        setSugIndex(-1);
      }).catch(() => setSuggestions([]));
    }, 250);
    return () => clearTimeout(t);
  }, [query, submitted]);

  const run = async (q: string, f: string, p: number) => {
    if (!q.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const items = await api.search(q, p, f);
      setResults((prev) => {
        const base = p === 1 ? [] : prev;
        const seen = new Set(base.map((x) => x.id));
        return [...base, ...items.filter((x) => !seen.has(x.id))];
      });
      setDone(items.length === 0);
      setPage(p);
    } catch (e) {
      setError(errText(e));
    } finally {
      setLoading(false);
    }
  };

  const submit = (q: string) => {
    setQuery(q);
    setSubmitted(q);
    setShowSug(false);
    setDone(false);
    setResults([]);
    run(q, filter, 1);
  };

  // live search while typing (debounced)
  useEffect(() => {
    if (query.trim().length < 2 || query === submitted) return;
    const t = setTimeout(() => {
      setSubmitted(query);
      setDone(false);
      run(query, filter, 1);
    }, 700);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query]);

  // infinite scroll
  useEffect(() => {
    const el = sentinel.current;
    if (!el) return;
    const io = new IntersectionObserver((entries) => {
      if (entries[0].isIntersecting && !loading && !done && submitted && results.length > 0) run(submitted, filter, page + 1);
    }, { rootMargin: "600px" });
    io.observe(el);
    return () => io.disconnect();
  });

  const setF = (f: string) => {
    setFilter(f);
    if (submitted) {
      setResults([]);
      setDone(false);
      run(submitted, f, 1);
    }
  };

  return (
    <div className="h-full overflow-y-auto px-8 pb-10 pt-8">
      <div className="relative mx-auto mb-6 max-w-3xl">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            submit(sugIndex >= 0 ? suggestions[sugIndex] : query);
          }}
        >
          <SearchIcon className="pointer-events-none absolute left-5 top-1/2 -translate-y-1/2 text-white/50" size={24} />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setShowSug(true);
            }}
            onFocus={() => setShowSug(true)}
            onBlur={() => setTimeout(() => setShowSug(false), 150)}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown" && suggestions.length) {
                e.preventDefault();
                setSugIndex((i) => Math.min(suggestions.length - 1, i + 1));
              } else if (e.key === "ArrowUp" && suggestions.length) {
                e.preventDefault();
                setSugIndex((i) => Math.max(-1, i - 1));
              } else if (e.key === "Escape") {
                setShowSug(false);
              }
            }}
            placeholder="Search movies and series…"
            aria-label="Search movies and series"
            className="h-16 w-full rounded-2xl border border-white/10 bg-panel pl-14 pr-14 text-xl outline-none transition placeholder:text-white/35 focus:border-white/40"
          />
          {query && (
            <button type="button" title="Clear" aria-label="Clear search" onClick={() => { setQuery(""); setSubmitted(""); setResults([]); inputRef.current?.focus(); }} className="absolute right-4 top-1/2 -translate-y-1/2 rounded-full p-2 text-white/60 hover:bg-white/10">
              <X size={20} />
            </button>
          )}
        </form>
        {showSug && suggestions.length > 0 && (
          <div className="absolute inset-x-0 top-[70px] z-30 overflow-hidden rounded-xl border border-white/10 bg-panel2 shadow-2xl">
            {suggestions.map((s, i) => (
              <button key={s + i} onMouseDown={(e) => { e.preventDefault(); submit(s); }} className={`flex w-full items-center gap-3 px-5 py-3 text-left hover:bg-white/10 ${i === sugIndex ? "bg-white/10" : ""}`}>
                <SearchIcon size={16} className="text-white/40" /> {s}
              </button>
            ))}
          </div>
        )}
        <div className="mt-4 flex justify-center gap-2">
          {[["all", "All"], ["movie", "Movies"], ["series", "Series"]].map(([k, label]) => (
            <button key={k} onClick={() => setF(k)} className={`rounded-full px-5 py-2 text-sm font-semibold transition ${filter === k ? "bg-white text-black" : "bg-white/10 text-white/80 hover:bg-white/20"}`}>
              {label}
            </button>
          ))}
        </div>
      </div>

      {!submitted && (
        <EmptyState icon={<SearchIcon size={36} />} title="Find something to watch" text="Type the name of a movie or series. Suggestions appear as you type." />
      )}
      {error && results.length === 0 && <ErrorState text={error} onRetry={() => run(submitted, filter, 1)} />}
      {submitted && loading && results.length === 0 && <SkeletonGrid />}
      {submitted && !loading && !error && results.length === 0 && (
        <EmptyState icon={<SearchX size={36} />} title={`No results for "${submitted}"`} text="Check the spelling, or try a shorter name." />
      )}
      {results.length > 0 && (
        <div className="fade-in grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-x-4 gap-y-6">
          {results.map((c, i) => (
            <PosterCard key={c.id} card={c} history={histMap.get(c.id)} navRow="grid" navCol={i} />
          ))}
        </div>
      )}
      <div ref={sentinel} className="flex h-20 items-center justify-center">
        {loading && results.length > 0 && <Spinner />}
      </div>
    </div>
  );
}
