import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Download, HardDrive } from "lucide-react";
import { api, errText, type Details, type Stream } from "../lib/api";
import { absIndex, epLabel, fmtBytes } from "../lib/format";
import { Button, Modal, Spinner } from "./ui";
import { useApp } from "../store/app";

export interface DlTarget {
  details: Details;
  items: { season: number; episode: number; title?: string | null }[];
}

interface Prepared {
  season: number;
  episode: number;
  title?: string | null;
  stream: Stream;
  sub: string | null;
}

function uniqueByHeight(list: Stream[]): Stream[] {
  const sorted = [...list].sort((a, b) => b.height - a.height);
  const out: Stream[] = [];
  for (const s of sorted) if (!out.some((o) => o.height === s.height)) out.push(s);
  return out;
}

function pickHeight(list: Stream[], height: number): Stream | undefined {
  const dl = list.filter((s) => s.downloadable);
  return dl.find((s) => s.height === height) ?? dl.filter((s) => s.height <= height).sort((a, b) => b.height - a.height)[0] ?? dl.sort((a, b) => a.height - b.height)[0];
}

async function findSub(d: Details, s: Stream, season: number, episode: number, lang: string | undefined): Promise<string | null> {
  if (!s.resourceId || !lang || lang === "Off") return null;
  try {
    const subs = await Promise.race([
      api.subtitles(d.id, s.resourceId, d.dubs.map((x) => x.subjectId), season, episode),
      new Promise<never>((_, rej) => setTimeout(() => rej(new Error("timeout")), 10000)),
    ]);
    return subs.find((x) => x.name.toLowerCase().includes(lang.toLowerCase()))?.url ?? null;
  } catch {
    return null;
  }
}

export function DownloadDialog({ target, onClose }: { target: DlTarget | null; onClose: () => void }) {
  const [step, setStep] = useState<"loading" | "choose" | "preparing" | "confirm" | "error">("loading");
  const [options, setOptions] = useState<Stream[]>([]);
  const [height, setHeight] = useState(0);
  const [err, setErr] = useState("");
  const [prepared, setPrepared] = useState<Prepared[]>([]);
  const [done, setDone] = useState(0);
  const [free, setFree] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const settings = useApp((s) => s.settings);
  const toast = useApp((s) => s.toast);
  const refreshDownloads = useApp((s) => s.refreshDownloads);
  const navigate = useNavigate();
  const isSeason = (target?.items.length ?? 0) > 1;

  useEffect(() => {
    if (!target) return;
    setStep("loading");
    setPrepared([]);
    setDone(0);
    setErr("");
    const d = target.details;
    const first = target.items[0];
    api.systemInfo().then((i) => setFree(i.freeBytes ?? null)).catch(() => {});
    api
      .streams(d.id, first.season, first.episode, absIndex(d.seasons, first.season, first.episode))
      .then((list) => {
        const dl = uniqueByHeight(list.filter((s) => s.downloadable));
        if (!dl.length) {
          setErr("Sorry, this title can't be downloaded right now. You can still watch it online.");
          setStep("error");
          return;
        }
        setOptions(dl);
        const pref = settings?.preferredQuality || 1080;
        setHeight((dl.find((s) => s.height <= pref) ?? dl[dl.length - 1]).height);
        setStep("choose");
      })
      .catch((e) => {
        setErr(errText(e));
        setStep("error");
      });
  }, [target]);

  if (!target) return null;
  const d = target.details;
  const chosen = options.find((o) => o.height === height);
  const total = isSeason ? prepared.reduce((a, p) => a + (p.stream.size ?? 0), 0) : chosen?.size ?? 0;
  const notEnough = free != null && total > 0 && total + 200 * 1024 * 1024 > free;

  const prepareSeason = async () => {
    setStep("preparing");
    const out: Prepared[] = [];
    let i = 0;
    const queue = [...target.items];
    const worker = async () => {
      while (queue.length) {
        const it = queue.shift()!;
        try {
          const list = await api.streams(d.id, it.season, it.episode, absIndex(d.seasons, it.season, it.episode));
          const s = pickHeight(list, height);
          if (s) out.push({ ...it, stream: s, sub: await findSub(d, s, it.season, it.episode, settings?.subtitleLanguage) });
        } catch {
          /* skip episodes that fail */
        }
        i++;
        setDone(i);
      }
    };
    await Promise.all([worker(), worker(), worker()]);
    out.sort((a, b) => a.season - b.season || a.episode - b.episode);
    setPrepared(out);
    setStep("confirm");
  };

  const start = async () => {
    setBusy(true);
    let list: Prepared[];
    if (isSeason) list = prepared;
    else {
      const it = target.items[0];
      list = chosen ? [{ ...it, stream: chosen, sub: await findSub(d, chosen, it.season, it.episode, settings?.subtitleLanguage) }] : [];
    }
    let added = 0;
    let lastErr = "";
    for (const p of list) {
      try {
        await api.downloadAdd({
          subjectId: d.id,
          title: d.title,
          year: d.year,
          poster: d.poster,
          mediaType: d.mediaType,
          season: p.season,
          episode: p.episode,
          absIndex: absIndex(d.seasons, p.season, p.episode),
          episodeTitle: p.title,
          height: p.stream.height,
          url: p.stream.url,
          headers: p.stream.headers,
          size: p.stream.size,
          subtitleUrl: p.sub,
          subtitleLang: settings?.subtitleLanguage,
        });
        added++;
      } catch (e) {
        lastErr = errText(e);
      }
    }
    setBusy(false);
    await refreshDownloads();
    onClose();
    if (added > 0) toast(added === 1 ? "Download started" : `${added} downloads added`, "success", { label: "View", run: () => navigate("/downloads") });
    if (lastErr) toast(lastErr, "error");
  };

  const heading = isSeason ? `Download Season ${target.items[0].season}` : d.mediaType === "series" ? `Download ${epLabel(target.items[0].season, target.items[0].episode)}` : `Download ${d.title}`;

  return (
    <Modal open onClose={onClose} title={heading} width={520}>
      {step === "loading" && (
        <div className="flex items-center gap-3 py-8 text-white/70">
          <Spinner /> Checking available qualities…
        </div>
      )}
      {step === "error" && <p className="py-4 text-white/75">{err}</p>}
      {step === "choose" && (
        <div>
          <p className="mb-3 text-white/60">Choose a quality. Higher quality looks better but uses more space.</p>
          <div className="flex flex-col gap-2">
            {options.map((o) => (
              <label key={o.height} className={`flex cursor-pointer items-center justify-between rounded-xl border px-4 py-3 transition ${o.height === height ? "border-white bg-white/10" : "border-white/10 hover:bg-white/5"}`}>
                <span className="flex items-center gap-3">
                  <input type="radio" name="q" checked={o.height === height} onChange={() => setHeight(o.height)} className="accent-[#e50914]" />
                  <span className="font-semibold">{o.height}p</span>
                  <span className="text-sm text-white/50">{o.height >= 1080 ? "Full HD" : o.height >= 720 ? "HD" : "Saves space"}</span>
                </span>
                {!isSeason && <span className="text-white/60">{fmtBytes(o.size)}</span>}
                {isSeason && o.size ? <span className="text-sm text-white/50">≈ {fmtBytes(o.size)} per episode</span> : null}
              </label>
            ))}
          </div>
          {isSeason && <p className="mt-3 text-sm text-white/55">{target.items.length} episodes will be added to your downloads.</p>}
        </div>
      )}
      {step === "preparing" && (
        <div className="py-6">
          <div className="mb-3 flex items-center gap-3 text-white/75">
            <Spinner /> Getting episodes ready… {done} of {target.items.length}
          </div>
          <div className="h-2 overflow-hidden rounded-full bg-white/10">
            <div className="h-full bg-brand transition-all" style={{ width: `${(done / target.items.length) * 100}%` }} />
          </div>
        </div>
      )}
      {step === "confirm" && (
        <p className="text-white/75">
          {prepared.length} of {target.items.length} episodes are ready to download at {height}p.
        </p>
      )}
      {(step === "choose" || step === "confirm") && (
        <div className="mt-5 rounded-xl bg-white/5 p-4 text-sm">
          <div className="flex items-center justify-between">
            <span className="flex items-center gap-2 text-white/60">
              <HardDrive size={16} /> Free space on your drive
            </span>
            <span className="font-semibold">{free != null ? fmtBytes(free) : "…"}</span>
          </div>
          {(!isSeason || step === "confirm") && total > 0 && (
            <div className="mt-2 flex items-center justify-between">
              <span className="text-white/60">This download needs</span>
              <span className="font-semibold">{fmtBytes(total)}</span>
            </div>
          )}
          {notEnough && (!isSeason || step === "confirm") && (
            <p className="mt-3 rounded-lg bg-brand/20 p-3 text-brand2">There isn't enough free space. Free up some space, pick a lower quality, or choose another download folder in Settings.</p>
          )}
        </div>
      )}
      <div className="mt-6 flex justify-end gap-3">
        <Button variant="ghost" onClick={onClose}>
          {step === "error" ? "Close" : "Cancel"}
        </Button>
        {step === "choose" && isSeason && (
          <Button variant="primary" onClick={prepareSeason}>
            Continue
          </Button>
        )}
        {((step === "choose" && !isSeason) || step === "confirm") && (
          <Button variant="primary" disabled={notEnough || busy || (step === "confirm" && prepared.length === 0)} onClick={start}>
            {busy ? <Spinner size={18} /> : <Download size={18} />} {isSeason ? `Download ${prepared.length} episodes` : "Download"}
          </Button>
        )}
      </div>
    </Modal>
  );
}
