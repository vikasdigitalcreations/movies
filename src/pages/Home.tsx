import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Info, Play } from "lucide-react";
import { api, errText, type HomeRow, type Card } from "../lib/api";
import { posterUrl } from "../lib/format";
import { PosterCard } from "../components/PosterCard";
import { Row } from "../components/Row";
import { Button, ErrorState, SkeletonRow } from "../components/ui";
import { useApp } from "../store/app";
import { useScrollMemory } from "../lib/hooks";

const cache = new Map<string, HomeRow[]>();

function Hero({ items }: { items: Card[] }) {
  const navigate = useNavigate();
  const [i, setI] = useState(0);
  const card = items[i % items.length];
  useEffect(() => {
    if (items.length < 2) return;
    const t = setInterval(() => setI((x) => x + 1), 9000);
    return () => clearInterval(t);
  }, [items.length]);
  if (!card) return null;
  return (
    <div className="relative mb-8 h-[46vh] min-h-[320px] overflow-hidden rounded-2xl">
      <img key={card.id} src={posterUrl(card.poster, 900)} alt="" className="fade-in absolute inset-0 h-full w-full scale-110 object-cover blur-2xl brightness-50" />
      <div className="absolute inset-0 bg-gradient-to-r from-ink via-ink/70 to-transparent" />
      <div className="absolute inset-0 bg-gradient-to-t from-ink via-transparent to-transparent" />
      <div className="relative flex h-full items-center gap-10 px-10">
        <img key={card.id + "p"} src={posterUrl(card.poster, 500)} alt="" className="fade-in hidden aspect-[2/3] h-[82%] rounded-xl object-cover shadow-2xl md:block" />
        <div className="fade-in max-w-xl" key={card.id + "t"}>
          <div className="mb-2 text-sm font-bold uppercase tracking-[.2em] text-brand2">Featured {card.mediaType === "series" ? "Series" : "Movie"}</div>
          <h1 className="mb-3 text-4xl font-black leading-tight tracking-tight lg:text-5xl">{card.title}</h1>
          {card.year && <div className="mb-6 text-white/60">{card.year}</div>}
          <div className="flex gap-3">
            <Button variant="primary" size="lg" onClick={() => navigate(`/title/${card.id}`, { state: { card, autoplay: true } })}>
              <Play size={22} fill="currentColor" /> Play
            </Button>
            <Button size="lg" onClick={() => navigate(`/title/${card.id}`, { state: { card } })}>
              <Info size={22} /> More Info
            </Button>
          </div>
        </div>
      </div>
      {items.length > 1 && (
        <div className="absolute bottom-4 right-6 flex gap-1.5">
          {items.slice(0, 10).map((c, k) => (
            <button key={c.id} aria-label={`Show ${c.title}`} onClick={() => setI(k)} className={`h-1.5 rounded-full transition-all ${k === i % items.length ? "w-6 bg-white" : "w-1.5 bg-white/40"}`} />
          ))}
        </div>
      )}
    </div>
  );
}

export function HomePage({ tab, title }: { tab: string; title?: string }) {
  const [rows, setRows] = useState<HomeRow[] | null>(cache.get(tab) ?? null);
  const [error, setError] = useState<string | null>(null);
  const history = useApp((s) => s.history);
  const scrollRef = useScrollMemory(`home-${tab}`, rows !== null);
  const navigate = useNavigate();

  const load = useCallback(() => {
    setError(null);
    api
      .home(tab)
      .then((r) => {
        cache.set(tab, r);
        setRows(r);
      })
      .catch((e) => setError(errText(e)));
  }, [tab]);

  useEffect(() => {
    setRows(cache.get(tab) ?? null);
    load();
  }, [tab, load]);

  const histMap = useMemo(() => {
    const m = new Map<string, (typeof history)[number]>();
    for (const h of history) if (!m.has(h.id)) m.set(h.id, h);
    return m;
  }, [history]);

  const continueItems = history.filter((h) => !h.completed && h.progress > 30).slice(0, 20);
  const hero = rows?.find((r) => r.kind === "hero")?.items ?? rows?.[0]?.items.slice(0, 8) ?? [];
  const others = rows?.filter((r) => r.kind !== "hero") ?? [];

  return (
    <div ref={scrollRef} className="h-full overflow-y-auto px-8 pb-10 pt-6">
      {title && <h1 className="mb-5 text-3xl font-extrabold tracking-tight">{title}</h1>}
      {error && !rows && <ErrorState text={error} onRetry={load} />}
      {!rows && !error && (
        <>
          <div className="skeleton mb-8 h-[46vh] min-h-[320px] rounded-2xl" />
          <SkeletonRow />
          <SkeletonRow />
        </>
      )}
      {rows && (
        <div className="fade-in">
          <Hero items={hero} />
          {tab === "0" && continueItems.length > 0 && (
            <Row title="Continue Watching" action={<button className="text-sm text-white/60 hover:text-white" onClick={() => navigate("/continue")}>See all</button>}>
              {continueItems.map((h, col) => (
                <PosterCard
                  key={h.id}
                  navRow="continue"
                  navCol={col}
                  width={160}
                  card={{ id: h.id, title: h.title, poster: h.poster, mediaType: h.mediaType, year: h.year }}
                  history={h}
                  subtitle={h.mediaType === "series" ? `S${h.season} E${h.episode}` : undefined}
                />
              ))}
            </Row>
          )}
          {others.map((row, ri) => (
            <Row key={row.title + ri} title={row.title}>
              {row.items.map((c, col) => (
                <PosterCard key={c.id} navRow={`r${ri}`} navCol={col} width={160} card={c} history={histMap.get(c.id)} />
              ))}
            </Row>
          ))}
        </div>
      )}
    </div>
  );
}
