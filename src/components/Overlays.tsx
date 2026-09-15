import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { AlertCircle, Bookmark, CheckCircle2, Download, Eye, Info, MousePointerClick, PlayCircle, Play, Search, Trash2, WifiOff, X } from "lucide-react";
import { api } from "../lib/api";
import { useApp } from "../store/app";
import { Button } from "./ui";

export function Toasts() {
  const toasts = useApp((s) => s.toasts);
  const dismiss = useApp((s) => s.dismissToast);
  return (
    <div className="pointer-events-none fixed bottom-6 left-1/2 z-[100] flex -translate-x-1/2 flex-col items-center gap-2">
      {toasts.map((t) => (
        <div key={t.id} role="status" className="fade-in pointer-events-auto flex min-w-[280px] max-w-[560px] items-center gap-3 rounded-xl border border-white/10 bg-[#202029] px-4 py-3 shadow-2xl">
          {t.kind === "success" ? <CheckCircle2 size={20} className="shrink-0 text-green-400" /> : t.kind === "error" ? <AlertCircle size={20} className="shrink-0 text-brand2" /> : <Info size={20} className="shrink-0 text-sky-400" />}
          <span className="flex-1 text-[15px]">{t.text}</span>
          {t.action && (
            <button
              className="rounded-lg bg-white/10 px-3 py-1 text-sm font-semibold hover:bg-white/20"
              onClick={() => {
                t.action!.run();
                dismiss(t.id);
              }}
            >
              {t.action.label}
            </button>
          )}
          <button className="rounded-full p-1 text-white/50 hover:bg-white/10" onClick={() => dismiss(t.id)} aria-label="Dismiss">
            <X size={16} />
          </button>
        </div>
      ))}
    </div>
  );
}

export function ContextMenu() {
  const menu = useApp((s) => s.contextMenu);
  const close = () => useApp.getState().openContextMenu(null);
  const favorites = useApp((s) => s.favorites);
  const toast = useApp((s) => s.toast);
  const refreshFavorites = useApp((s) => s.refreshFavorites);
  const refreshHistory = useApp((s) => s.refreshHistory);
  const navigate = useNavigate();

  useEffect(() => {
    if (!menu) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
    const onScroll = () => close();
    window.addEventListener("keydown", onKey);
    window.addEventListener("wheel", onScroll, { passive: true });
    window.addEventListener("resize", onScroll);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("wheel", onScroll);
      window.removeEventListener("resize", onScroll);
    };
  }, [menu]);

  if (!menu) return null;
  const { card, history } = menu;
  const inList = favorites.some((f) => f.id === card.id);
  const x = Math.min(menu.x, window.innerWidth - 250);
  const y = Math.min(menu.y, window.innerHeight - 260);

  const item = (icon: React.ReactNode, label: string, run: () => void, autoFocus = false) => (
    <button
      autoFocus={autoFocus}
      className="flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left text-[15px] hover:bg-white/10 focus:bg-white/10"
      onClick={() => {
        close();
        run();
      }}
    >
      {icon}
      {label}
    </button>
  );

  return (
    <div className="fixed inset-0 z-[90]" onMouseDown={close} onContextMenu={(e) => { e.preventDefault(); close(); }}>
      <div className="fade-in absolute w-60 rounded-xl border border-white/10 bg-[#1c1c24] p-1.5 shadow-2xl" style={{ left: x, top: y }} onMouseDown={(e) => e.stopPropagation()}>
        <div className="truncate px-3 pb-1.5 pt-1 text-xs font-bold uppercase tracking-wider text-white/40">{card.title}</div>
        {item(<Play size={18} />, history && !history.completed ? "Resume" : "Play", () => navigate(`/title/${card.id}`, { state: { card, autoplay: true } }), true)}
        {item(<Info size={18} />, "More info", () => navigate(`/title/${card.id}`, { state: { card } }))}
        {item(<Bookmark size={18} />, inList ? "Remove from My List" : "Add to My List", async () => {
          const now = await api.favoritesToggle(card);
          refreshFavorites();
          toast(now ? "Added to My List" : "Removed from My List", "success");
        })}
        {item(<Download size={18} />, "Download", () => navigate(`/title/${card.id}`, { state: { card, download: true } }))}
        {item(<Eye size={18} />, "Mark as watched", async () => {
          await api.historyMarkWatched({
            id: card.id,
            title: card.title,
            poster: card.poster,
            mediaType: card.mediaType,
            year: card.year,
            season: history?.season ?? (card.mediaType === "series" ? 1 : 0),
            episode: history?.episode ?? (card.mediaType === "series" ? 1 : 0),
          });
          refreshHistory();
          toast("Marked as watched", "success");
        })}
        {history && item(<Trash2 size={18} />, "Remove from Continue Watching", async () => {
          await api.historyRemove(card.id);
          refreshHistory();
          toast("Removed from Continue Watching", "success");
        })}
      </div>
    </div>
  );
}

export function OfflineBanner() {
  const online = useApp((s) => s.online);
  const setOnline = useApp((s) => s.setOnline);
  const navigate = useNavigate();

  useEffect(() => {
    const on = () => setOnline(true);
    const off = () => setOnline(false);
    window.addEventListener("online", on);
    window.addEventListener("offline", off);
    return () => {
      window.removeEventListener("online", on);
      window.removeEventListener("offline", off);
    };
  }, [setOnline]);

  useEffect(() => {
    let alive = true;
    const check = async () => {
      const ok = navigator.onLine ? await api.checkOnline().catch(() => false) : false;
      if (alive) setOnline(ok);
    };
    check();
    const t = window.setInterval(check, online ? 60000 : 8000);
    return () => {
      alive = false;
      window.clearInterval(t);
    };
  }, [online, setOnline]);

  if (online) return null;
  return (
    <div role="alert" className="flex items-center justify-center gap-3 bg-yellow-500 px-4 py-2 text-[15px] font-semibold text-black">
      <WifiOff size={18} /> You're offline. Your downloads still work.
      <button className="rounded-md bg-black/15 px-3 py-0.5 hover:bg-black/25" onClick={() => navigate("/downloads")}>
        Go to Downloads
      </button>
    </div>
  );
}

const STEPS = [
  { icon: Search, title: "Find something to watch", text: "Use Search, or browse Home, Movies, Series and Anime from the menu on the left." },
  { icon: MousePointerClick, title: "Open a title", text: "Click any poster to see the story, rating and episodes. Right-click for quick actions." },
  { icon: PlayCircle, title: "Press Play", text: "Videos play right here with subtitles. Press F for full screen and ? to see all shortcuts." },
  { icon: Download, title: "Download for later", text: "Press Download to save a movie or a whole season, then watch it without internet from Downloads." },
];

export function WelcomeTour() {
  const open = useApp((s) => s.tourOpen);
  const setOpen = useApp((s) => s.setTourOpen);
  const save = useApp((s) => s.saveSettings);
  const [step, setStep] = useState(0);

  useEffect(() => {
    if (open) setStep(0);
  }, [open]);

  if (!open) return null;
  const s = STEPS[step];
  const finish = () => {
    setOpen(false);
    save({ tourDone: true });
  };
  return (
    <div className="fixed inset-0 z-[95] flex items-center justify-center bg-black/80 p-4 backdrop-blur-sm">
      <div className="fade-in w-full max-w-lg rounded-3xl border border-white/10 bg-panel p-8 text-center shadow-2xl" key={step}>
        {step === 0 && <div className="mb-2 text-sm font-bold uppercase tracking-[.25em] text-brand2">Welcome to MovieBox</div>}
        <div className="mx-auto mb-5 mt-3 flex h-24 w-24 items-center justify-center rounded-full bg-brand/15 text-brand2">
          <s.icon size={48} />
        </div>
        <h2 className="text-2xl font-extrabold">{s.title}</h2>
        <p className="mx-auto mt-3 max-w-sm text-[17px] text-white/70">{s.text}</p>
        <div className="mt-6 flex justify-center gap-2">
          {STEPS.map((_, i) => (
            <span key={i} className={`h-2 rounded-full transition-all ${i === step ? "w-8 bg-white" : "w-2 bg-white/30"}`} />
          ))}
        </div>
        <div className="mt-8 flex items-center justify-between">
          <Button variant="ghost" onClick={finish}>
            Skip
          </Button>
          <div className="flex gap-2">
            {step > 0 && <Button onClick={() => setStep(step - 1)}>Back</Button>}
            <Button variant="primary" autoFocus onClick={() => (step === STEPS.length - 1 ? finish() : setStep(step + 1))}>
              {step === STEPS.length - 1 ? "Start watching" : "Next"}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
