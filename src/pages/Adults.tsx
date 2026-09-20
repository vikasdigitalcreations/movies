import { useCallback, useEffect, useRef, useState } from "react";
import { Lock, Search as SearchIcon, X } from "lucide-react";
import { api, type AdultSource, type Card as CardT } from "../lib/api";
import { Button, EmptyState, ErrorState, PageHeader, SkeletonGrid, Spinner } from "../components/ui";
import { usePlayNow } from "../store/player";
import { useApp } from "../store/app";

const SOURCES: { id: AdultSource; label: string; note: string }[] = [
  { id: "redgifs", label: "RedGifs", note: "Short clips. Play here, and downloadable." },
  { id: "eporner", label: "Eporner", note: "Full videos. Open in the site's own player." },
];

/**
 * The PIN gate. It is asked for once per launch, never remembered, and it guards the
 * section rather than the data -- anyone with the PC could edit the settings file. That
 * is the right strength for "don't let this open by accident in front of someone".
 */
function PinGate({ onPass }: { onPass: () => void }) {
  const [pin, setPin] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (busy || !pin) return;
    setBusy(true);
    setErr(null);
    try {
      if (await api.pinVerify(pin)) onPass();
      else {
        setErr("That PIN isn't right.");
        setPin("");
      }
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex min-h-[60vh] items-center justify-center">
      <div className="w-[320px] rounded-2xl border border-white/8 bg-panel p-6 text-center">
        <div className="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-full bg-white/10">
          <Lock size={22} />
        </div>
        <p className="mb-5 text-white/70">Enter your PIN to open this section.</p>
        <input
          autoFocus
          type="password"
          inputMode="numeric"
          value={pin}
          onChange={(e) => setPin(e.target.value.replace(/\D/g, "").slice(0, 12))}
          onKeyDown={(e) => e.key === "Enter" && submit()}
          className="w-full rounded-xl border border-white/10 bg-black/40 px-3 py-2 text-center text-2xl tracking-[0.5em] outline-none focus:border-white/25"
        />
        {err && <p className="mt-3 text-sm text-red-400">{err}</p>}
        <Button variant="primary" className="mt-5 w-full" onClick={submit} disabled={busy || pin.length < 4}>
          {busy ? <Spinner size={16} /> : "Unlock"}
        </Button>
      </div>
    </div>
  );
}

/** Eporner will not serve its files outside its own player, so we show that player. */
function EmbedView({ url, onClose }: { url: string; onClose: () => void }) {
  useEffect(() => {
    const esc = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [onClose]);

  return (
    <div className="fixed inset-0 z-50 flex flex-col bg-black">
      <div className="flex items-center justify-between px-4 py-2">
        <span className="text-sm text-white/50">Playing on Eporner — press Esc to close</span>
        <button onClick={onClose} className="rounded-lg p-2 text-white/60 transition hover:bg-white/10 hover:text-white" title="Close">
          <X size={18} />
        </button>
      </div>
      <iframe
        src={url}
        className="flex-1 border-0"
        allow="fullscreen; autoplay"
        referrerPolicy="no-referrer"
        sandbox="allow-scripts allow-same-origin allow-presentation"
        title="Video"
      />
    </div>
  );
}

export function AdultsPage() {
  const [unlocked, setUnlocked] = useState(false);
  const [source, setSource] = useState<AdultSource>("redgifs");
  const [query, setQuery] = useState("");
  const [typed, setTyped] = useState("");
  const [items, setItems] = useState<CardT[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [more, setMore] = useState(false);
  const [embed, setEmbed] = useState<string | null>(null);
  const [opening, setOpening] = useState<string | null>(null);
  const playNow = usePlayNow();
  const toast = useApp((s) => s.toast);
  const reqId = useRef(0);

  const load = useCallback(
    async (p: number, replace: boolean) => {
      const mine = ++reqId.current;
      if (replace) setItems(null);
      setErr(null);
      try {
        const got = await api.adultSearch(source, query, p, true);
        if (mine !== reqId.current) return;
        setItems((cur) => (replace || !cur ? got : [...cur, ...got]));
        setMore(got.length >= 20);
      } catch (e) {
        if (mine !== reqId.current) return;
        setErr(String(e));
        setItems([]);
      }
    },
    [source, query],
  );

  useEffect(() => {
    if (!unlocked) return;
    setPage(1);
    load(1, true);
  }, [unlocked, load]);

  const open = async (c: CardT) => {
    if (opening) return;
    setOpening(c.id);
    try {
      const pb = await api.adultPlayback(c.id, true);
      if (pb.kind === "embed" && pb.embedUrl) {
        setEmbed(pb.embedUrl);
      } else if (pb.stream) {
        playNow({
          ref: { id: c.id, title: c.title, year: c.year ?? null, poster: c.poster, mediaType: "movie", season: 0, episode: 0 },
          seasons: [],
          dubIds: [],
          startAt: 0,
          directStream: pb.stream,
        });
      }
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setOpening(null);
    }
  };

  if (!unlocked) return <PinGate onPass={() => setUnlocked(true)} />;

  return (
    <div>
      <PageHeader title="Adults" subtitle="18+. Hidden behind your PIN, and never part of search or Home." />

      <div className="mb-5 flex flex-wrap items-center gap-2">
        {SOURCES.map((s) => (
          <button
            key={s.id}
            onClick={() => setSource(s.id)}
            title={s.note}
            className={`rounded-xl px-3 py-1.5 text-sm font-semibold transition ${
              source === s.id ? "bg-brand text-white" : "bg-white/8 text-white/60 hover:bg-white/14"
            }`}
          >
            {s.label}
          </button>
        ))}
        <div className="ml-auto flex items-center gap-2">
          <div className="flex items-center gap-2 rounded-xl border border-white/10 bg-black/30 px-3 py-1.5">
            <SearchIcon size={15} className="text-white/35" />
            <input
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && setQuery(typed.trim())}
              placeholder="Search…"
              className="w-44 bg-transparent text-sm outline-none placeholder:text-white/30"
            />
            {typed && (
              <button
                onClick={() => {
                  setTyped("");
                  setQuery("");
                }}
                className="text-white/40 hover:text-white"
                title="Clear"
              >
                <X size={14} />
              </button>
            )}
          </div>
        </div>
      </div>

      <p className="mb-5 text-sm text-white/40">{SOURCES.find((s) => s.id === source)?.note}</p>

      {items === null ? (
        <SkeletonGrid count={18} />
      ) : err ? (
        <ErrorState text={err} onRetry={() => load(1, true)} />
      ) : items.length === 0 ? (
        <EmptyState icon={<SearchIcon size={30} />} title="Nothing found" text="Try a different word, or the other source." />
      ) : (
        <>
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6">
            {items.map((c) => (
              <button
                key={c.id}
                onClick={() => open(c)}
                className="group text-left"
                title={c.title}
                disabled={opening !== null}
              >
                <div className="relative aspect-video overflow-hidden rounded-xl bg-white/5">
                  {c.poster && (
                    <img
                      src={c.poster}
                      alt=""
                      loading="lazy"
                      referrerPolicy="no-referrer"
                      className="h-full w-full object-cover transition group-hover:scale-105"
                    />
                  )}
                  {c.year && (
                    <span className="absolute bottom-1.5 right-1.5 rounded-md bg-black/75 px-1.5 py-0.5 text-xs tabular-nums">{c.year}</span>
                  )}
                  {opening === c.id && (
                    <div className="absolute inset-0 grid place-items-center bg-black/60">
                      <Spinner size={22} />
                    </div>
                  )}
                </div>
                <div className="mt-1.5 line-clamp-2 text-sm text-white/70 group-hover:text-white">{c.title}</div>
              </button>
            ))}
          </div>
          {more && (
            <div className="mt-8 flex justify-center">
              <Button
                onClick={() => {
                  const n = page + 1;
                  setPage(n);
                  load(n, false);
                }}
              >
                Show more
              </Button>
            </div>
          )}
        </>
      )}

      {embed && <EmbedView url={embed} onClose={() => setEmbed(null)} />}
    </div>
  );
}
