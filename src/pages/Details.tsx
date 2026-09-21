import { useEffect, useMemo, useRef, useState } from "react";
import { useLocation, useNavigate, useParams } from "react-router-dom";
import { ArrowLeft, Bookmark, BookmarkCheck, Check, Clock, Download, Play, Star, Languages, MonitorPlay } from "lucide-react";
import { api, errText, type Card, type Details } from "../lib/api";
import { absIndex, epLabel, fmtTime, posterUrl } from "../lib/format";
import { Button, ErrorState, IconButton } from "../components/ui";
import { DownloadDialog, type DlTarget } from "../components/DownloadDialog";
import { useApp } from "../store/app";
import { neighbor, usePlayNow } from "../store/player";

export function DetailsPage() {
  const { id = "" } = useParams();
  const location = useLocation();
  const navigate = useNavigate();
  const st = (location.state ?? {}) as { card?: Card; autoplay?: boolean; download?: boolean };
  const [d, setD] = useState<Details | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [season, setSeason] = useState(1);
  const [watched, setWatched] = useState<boolean[]>([]);
  const [fav, setFav] = useState(false);
  const [dl, setDl] = useState<DlTarget | null>(null);
  const [quality, setQuality] = useState<number | null>(null);
  const history = useApp((s) => s.history);
  const toast = useApp((s) => s.toast);
  const preferredQuality = useApp((s) => s.settings?.preferredQuality);
  const refreshFavorites = useApp((s) => s.refreshFavorites);
  const playNow = usePlayNow();
  const autoDone = useRef(false);

  const hist = history.find((h) => h.id === id);

  const load = () => {
    setError(null);
    api
      .details(id)
      .then((x) => {
        setD(x);
        setFav(x.isFavorite);
        const h = history.find((hh) => hh.id === x.id);
        setSeason(h && h.season > 0 && x.seasons.some((s) => s.number === h.season) ? h.season : x.seasons[0]?.number ?? 1);
      })
      .catch((e) => setError(errText(e)));
  };

  useEffect(() => {
    setD(null);
    autoDone.current = false;
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  const seasonObj = d?.seasons.find((s) => s.number === season);

  useEffect(() => {
    if (!d || !seasonObj) return;
    api
      .historyWatched(d.id, seasonObj.episodes.map((e) => [season, e.number] as [number, number]))
      .then(setWatched)
      .catch(() => {});
  }, [d, season, seasonObj, history]);

  const play = (s: number, e: number, title?: string | null, startAt: number | null = null) => {
    if (!d) return;
    playNow({
      ref: { id: d.id, title: d.title, poster: d.poster, mediaType: d.mediaType, year: d.year, season: s, episode: e },
      seasons: d.seasons,
      dubIds: d.dubs.map((x) => x.subjectId),
      episodeTitle: title,
      startAt,
      qualityOverride: quality,
    });
  };

  const main = useMemo(() => {
    if (!d) return null;
    if (d.mediaType === "series" && d.seasons.length) {
      if (hist && hist.season > 0) {
        if (!hist.completed && hist.progress > 30) {
          return { label: `Resume ${epLabel(hist.season, hist.episode)} · ${fmtTime(hist.progress)}`, target: { season: hist.season, episode: hist.episode }, run: () => play(hist.season, hist.episode, null, Math.max(0, hist.progress - 5)) };
        }
        const n = neighbor(d.seasons, hist.season, hist.episode, 1);
        if (n) return { label: `Play ${epLabel(n.season, n.episode)}`, target: { season: n.season, episode: n.episode }, run: () => play(n.season, n.episode, n.title, 0) };
      }
      const s0 = [...d.seasons].sort((a, b) => a.number - b.number)[0];
      const e0 = s0.episodes[0];
      return { label: "Play", target: { season: s0.number, episode: e0?.number ?? 1 }, run: () => play(s0.number, e0?.number ?? 1, e0?.title, null) };
    }
    if (hist && !hist.completed && hist.progress > 30) {
      return { label: `Resume · ${fmtTime(hist.progress)}`, target: { season: 0, episode: 0 }, run: () => play(0, 0, null, Math.max(0, hist.progress - 5)) };
    }
    return { label: "Play", target: { season: 0, episode: 0 }, run: () => play(0, 0, null, null) };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [d, hist, quality]);

  // Start looking for the stream now, while the person reads the page, so Play has it
  // already. The player asks for exactly what the main button will play.
  const targetSeason = main?.target.season;
  const targetEpisode = main?.target.episode;
  useEffect(() => {
    if (!d || targetSeason === undefined || targetEpisode === undefined) return;
    api.prefetchStreams(d.id, targetSeason, targetEpisode, absIndex(d.seasons, targetSeason, targetEpisode), d.title, d.year, quality ?? preferredQuality ?? 0);
  }, [d, targetSeason, targetEpisode, quality, preferredQuality]);

  useEffect(() => {
    if (!d || autoDone.current) return;
    if (!st.autoplay && !st.download) return;
    autoDone.current = true;
    // "Play"/"Download" straight from a poster is a one-shot intent. Forget it right
    // away, otherwise coming back here from the player replays it and the video
    // reopens instead of the page staying put.
    navigate(location.pathname, { replace: true, state: st.card ? { card: st.card } : null });
    if (st.autoplay) {
      main?.run();
    } else if (d.mediaType === "movie") {
      setDl({ details: d, items: [{ season: 0, episode: 0 }] });
    } else if (seasonObj) {
      setDl({ details: d, items: seasonObj.episodes.map((e) => ({ season, episode: e.number, title: e.title })) });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [d]);

  const toggleFav = async () => {
    if (!d) return;
    const now = await api.favoritesToggle({ id: d.id, title: d.title, year: d.year, poster: d.poster, mediaType: d.mediaType });
    setFav(now);
    refreshFavorites();
    toast(now ? "Added to My List" : "Removed from My List", "success");
  };

  if (error) {
    return (
      <div className="h-full overflow-y-auto px-8 pt-6">
        <Button variant="ghost" onClick={() => navigate(-1)}>
          <ArrowLeft size={20} /> Back
        </Button>
        <ErrorState text={error} onRetry={load} />
      </div>
    );
  }

  const card = st.card;
  const poster = d?.poster ?? card?.poster;
  const title = d?.title ?? card?.title ?? "";

  return (
    <div className="relative h-full overflow-y-auto overflow-x-hidden">
      {poster && <img src={posterUrl(poster, 900)} alt="" className="pointer-events-none absolute inset-x-0 top-0 h-[70vh] w-full scale-110 object-cover opacity-40 blur-3xl" />}
      <div className="pointer-events-none absolute inset-x-0 top-0 h-[70vh] bg-gradient-to-b from-transparent via-ink/60 to-ink" />
      <div className="relative px-8 pb-12 pt-5">
        <IconButton title="Back" onClick={() => navigate(-1)} className="mb-4 bg-black/40">
          <ArrowLeft size={22} />
        </IconButton>
        <div className="flex flex-col gap-8 md:flex-row">
          <div className="w-[240px] shrink-0">
            {poster ? <img src={posterUrl(poster, 500)} alt="" className="aspect-[2/3] w-full rounded-2xl object-cover shadow-2xl" /> : <div className="skeleton aspect-[2/3] w-full rounded-2xl" />}
          </div>
          <div className="min-w-0 flex-1">
            <h1 className="text-4xl font-black leading-tight tracking-tight">{title}</h1>
            {!d && (
              <div className="mt-4 space-y-3">
                <div className="skeleton h-5 w-64 rounded" />
                <div className="skeleton h-12 w-80 rounded-lg" />
                <div className="skeleton h-20 w-full max-w-2xl rounded" />
              </div>
            )}
            {d && (
              <div className="fade-in">
                <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-2 text-white/70">
                  {d.year && <span>{d.year}</span>}
                  {d.imdbRating && (
                    <span className="flex items-center gap-1 font-semibold text-yellow-400">
                      <Star size={16} fill="currentColor" /> {d.imdbRating}
                    </span>
                  )}
                  {d.duration && (
                    <span className="flex items-center gap-1">
                      <Clock size={16} /> {d.duration}
                    </span>
                  )}
                  <span className="rounded border border-white/25 px-1.5 text-xs font-semibold uppercase">{d.mediaType === "series" ? `${d.seasons.length} season${d.seasons.length === 1 ? "" : "s"}` : "Movie"}</span>
                  {d.genres.map((g) => (
                    <span key={g} className="rounded-full bg-white/10 px-3 py-0.5 text-sm">
                      {g}
                    </span>
                  ))}
                </div>

                <div className="mt-6 flex flex-wrap items-center gap-3">
                  <Button variant="primary" size="lg" onClick={main?.run} autoFocus>
                    <Play size={22} fill="currentColor" /> {main?.label}
                  </Button>
                  {main?.label.startsWith("Resume") && (
                    <Button size="lg" onClick={() => (d.mediaType === "series" && hist ? play(hist.season, hist.episode, null, 0) : play(0, 0, null, 0))}>
                      Start over
                    </Button>
                  )}
                  <Button size="lg" onClick={toggleFav} title={fav ? "Remove from My List" : "Add to My List"}>
                    {fav ? <BookmarkCheck size={22} /> : <Bookmark size={22} />} {fav ? "In My List" : "My List"}
                  </Button>
                  {d.mediaType === "movie" && (
                    <Button size="lg" onClick={() => setDl({ details: d, items: [{ season: 0, episode: 0 }] })}>
                      <Download size={22} /> Download
                    </Button>
                  )}
                </div>

                <div className="mt-5 flex flex-wrap items-center gap-4 text-sm">
                  {d.dubs.length > 1 && (
                    <label className="flex items-center gap-2 text-white/70">
                      <Languages size={18} /> Audio language
                      <select value={d.id} onChange={(e) => navigate(`/title/${e.target.value}`, { replace: true })} className="rounded-lg border border-white/15 bg-panel2 px-3 py-2 text-white outline-none">
                        {d.dubs.map((x) => (
                          <option key={x.subjectId} value={x.subjectId}>
                            {x.label}
                          </option>
                        ))}
                      </select>
                    </label>
                  )}
                  <label className="flex items-center gap-2 text-white/70">
                    <MonitorPlay size={18} /> Quality
                    <select value={quality ?? ""} onChange={(e) => setQuality(e.target.value === "" ? null : Number(e.target.value))} className="rounded-lg border border-white/15 bg-panel2 px-3 py-2 text-white outline-none">
                      <option value="">Auto (Best)</option>
                      <option value="1080">1080p</option>
                      <option value="720">720p</option>
                      <option value="480">480p</option>
                      <option value="360">360p</option>
                    </select>
                  </label>
                </div>

                {d.description && <p className="mt-6 max-w-3xl text-[17px] leading-relaxed text-white/85">{d.description}</p>}
                <div className="mt-4 max-w-3xl space-y-1 text-sm text-white/55">
                  {d.stars && (
                    <div>
                      <span className="text-white/40">Cast: </span>
                      {d.stars}
                    </div>
                  )}
                  {d.director && (
                    <div>
                      <span className="text-white/40">Director: </span>
                      {d.director}
                    </div>
                  )}
                </div>
              </div>
            )}
          </div>
        </div>

        {d && d.mediaType === "series" && d.seasons.length > 0 && (
          <section className="fade-in mt-10">
            <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
              <div className="no-scrollbar flex gap-2 overflow-x-auto">
                {[...d.seasons]
                  .sort((a, b) => a.number - b.number)
                  .map((s) => (
                    <button key={s.number} onClick={() => setSeason(s.number)} className={`shrink-0 rounded-full px-5 py-2 font-semibold transition ${s.number === season ? "bg-white text-black" : "bg-white/10 text-white/80 hover:bg-white/20"}`}>
                      Season {s.number}
                    </button>
                  ))}
              </div>
              {seasonObj && (
                <Button onClick={() => setDl({ details: d, items: seasonObj.episodes.map((e) => ({ season, episode: e.number, title: e.title })) })}>
                  <Download size={18} /> Download whole season
                </Button>
              )}
            </div>
            <div className="flex flex-col gap-2">
              {seasonObj?.episodes.map((e, i) => {
                const isCurrent = hist && hist.season === season && hist.episode === e.number;
                const pct = isCurrent && hist?.duration ? Math.min(100, (hist.progress / hist.duration) * 100) : 0;
                const done = watched[i];
                return (
                  <div
                    key={e.number}
                    style={{ contentVisibility: "auto", containIntrinsicSize: "72px" }}
                    className={`group flex items-center gap-4 rounded-xl border px-4 py-3 transition hover:bg-white/6 ${isCurrent ? "border-white/25 bg-white/5" : "border-white/6"}`}
                  >
                    <div className="w-10 text-center text-2xl font-bold text-white/40">{e.number}</div>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2 font-semibold">
                        <span className="truncate">{e.title || `Episode ${e.number}`}</span>
                        {done && (
                          <span className="flex items-center gap-1 rounded-full bg-green-500/15 px-2 py-0.5 text-xs text-green-400">
                            <Check size={12} strokeWidth={3} /> Watched
                          </span>
                        )}
                      </div>
                      {pct > 0 && !done && (
                        <div className="mt-2 h-1 w-full max-w-sm overflow-hidden rounded-full bg-white/15">
                          <div className="h-full bg-brand" style={{ width: `${pct}%` }} />
                        </div>
                      )}
                    </div>
                    <Button size="sm" variant={isCurrent ? "primary" : "secondary"} data-nav-row="eps" onClick={() => play(season, e.number, e.title, isCurrent && !hist?.completed ? null : 0)} title={`Play episode ${e.number}`}>
                      <Play size={16} fill="currentColor" /> {isCurrent && !hist?.completed && (hist?.progress ?? 0) > 30 ? "Resume" : "Play"}
                    </Button>
                    <IconButton title={`Download episode ${e.number}`} onClick={() => setDl({ details: d, items: [{ season, episode: e.number, title: e.title }] })}>
                      <Download size={20} />
                    </IconButton>
                  </div>
                );
              })}
            </div>
          </section>
        )}
      </div>
      <DownloadDialog target={dl} onClose={() => setDl(null)} />
    </div>
  );
}
