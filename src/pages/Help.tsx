import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { ChevronDown, FileText, LifeBuoy, PlayCircle, Search, Download, MousePointerClick, Wifi, Trash2, RefreshCw, RotateCcw } from "lucide-react";
import { api } from "../lib/api";
import { APP_SHORTCUTS, PLAYER_SHORTCUTS } from "../lib/shortcuts";
import { Button, PageHeader } from "../components/ui";
import { useApp } from "../store/app";

const FAQ: [string, string][] = [
  ["Do I need to install VLC or anything else?", "No. MovieBox has its own video player built in. Just open a title and press Play."],
  ["Why does a video say it's buffering?", "Your internet is slower than the video. Give it a moment — the player keeps a large buffer and steps down to a lower quality on its own when the connection is slow."],
  ["How do I watch without internet?", "Open a movie or series, press Download, and when it's finished go to Downloads and press Play."],
  ["Where are my downloads saved?", "In your Downloads folder, inside a MovieBox folder. You can change this in Settings."],
  ["How do I turn subtitles on or off?", "While watching, click the speech-bubble icon or press C. Choose your usual language in Settings."],
  ["Can I change the audio language?", "On a title's page, use the Audio language menu (when other languages exist). In the player, press A to switch audio tracks."],
  ["The picture has black bars.", "Press W in the player to switch between Fit, Fill and Stretch."],
  ["Does MovieBox track me?", "No. There are no accounts and no tracking. Your history and lists stay on this PC."],
];

export function HelpPage() {
  const navigate = useNavigate();
  const toast = useApp((s) => s.toast);
  const setTourOpen = useApp((s) => s.setTourOpen);
  const [open, setOpen] = useState<number | null>(0);
  const [checking, setChecking] = useState(false);

  return (
    <div className="h-full overflow-y-auto px-8 pb-12 pt-8">
      <div className="max-w-3xl">
        <PageHeader title="Help" subtitle="Everything you need to start watching." right={<Button onClick={() => setTourOpen(true)}><PlayCircle size={18} /> Show the welcome tour</Button>} />

        <section className="mb-8 grid grid-cols-1 gap-3 md:grid-cols-2">
          {[
            [Search, "1. Find something", "Use Search or browse Home, Movies and Series."],
            [MousePointerClick, "2. Open it", "Click a poster to see the story, seasons and episodes."],
            [PlayCircle, "3. Press Play", "It plays right here, in the best quality, with subtitles."],
            [Download, "4. Download for later", "Press Download to watch without internet."],
          ].map(([Icon, t, d]) => {
            const I = Icon as typeof Search;
            return (
              <div key={t as string} className="flex gap-4 rounded-2xl border border-white/6 bg-panel p-5">
                <I size={28} className="shrink-0 text-brand2" />
                <div>
                  <div className="font-bold">{t as string}</div>
                  <div className="text-sm text-white/60">{d as string}</div>
                </div>
              </div>
            );
          })}
        </section>

        <section className="mb-8 rounded-2xl border border-yellow-500/20 bg-yellow-500/5 p-6">
          <h2 className="mb-1 flex items-center gap-2 text-xl font-bold">
            <LifeBuoy size={22} className="text-yellow-400" /> Something's not working?
          </h2>
          <p className="mb-4 text-white/60">Try these in order. Most problems are fixed by step 1 or 2.</p>
          <ol className="space-y-4">
            <li className="flex items-start gap-4">
              <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-white/10 font-bold">1</span>
              <div className="flex-1">
                <div className="font-semibold">Check your internet</div>
                <div className="text-sm text-white/55">Make sure other websites open. Wi-Fi far from the router can be slow.</div>
              </div>
              <Button
                size="sm"
                disabled={checking}
                onClick={async () => {
                  setChecking(true);
                  const ok = await api.checkOnline().catch(() => false);
                  setChecking(false);
                  toast(ok ? "Your internet is working." : "No internet connection found.", ok ? "success" : "error");
                }}
              >
                <Wifi size={16} /> {checking ? "Checking…" : "Check now"}
              </Button>
            </li>
            <li className="flex items-start gap-4">
              <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-white/10 font-bold">2</span>
              <div className="flex-1">
                <div className="font-semibold">Try another source</div>
                <div className="text-sm text-white/55">If a video won't start, the player offers "Try another source". If it keeps buffering, wait a few seconds — the picture drops to a lower quality by itself and sharpens again later.</div>
              </div>
            </li>
            <li className="flex items-start gap-4">
              <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-white/10 font-bold">3</span>
              <div className="flex-1">
                <div className="font-semibold">Clear the cache</div>
                <div className="text-sm text-white/55">Fixes titles, posters or lists that won't load.</div>
              </div>
              <Button size="sm" onClick={() => api.clearCache().then(() => toast("Cache cleared", "success"))}>
                <Trash2 size={16} /> Clear cache
              </Button>
            </li>
            <li className="flex items-start gap-4">
              <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-white/10 font-bold">4</span>
              <div className="flex-1">
                <div className="font-semibold">Restart MovieBox</div>
                <div className="text-sm text-white/55">Close the app and open it again. Unfinished downloads continue automatically.</div>
              </div>
              <Button size="sm" onClick={() => window.location.reload()}>
                <RefreshCw size={16} /> Reload
              </Button>
            </li>
          </ol>
          <div className="mt-6 flex flex-wrap items-center gap-3 border-t border-white/10 pt-5">
            <Button onClick={() => api.openLogs().catch(() => toast("Couldn't open the log folder.", "error"))}>
              <FileText size={18} /> Report a problem
            </Button>
            <span className="text-sm text-white/55">Opens the folder with the log file. Send the newest file to whoever helps you.</span>
          </div>
        </section>

        <section className="mb-8">
          <h2 className="mb-3 text-xl font-bold">Questions</h2>
          <div className="divide-y divide-white/6 rounded-2xl border border-white/6 bg-panel">
            {FAQ.map(([q, a], i) => (
              <div key={q}>
                <button onClick={() => setOpen(open === i ? null : i)} className="flex w-full items-center justify-between gap-4 px-5 py-4 text-left font-semibold hover:bg-white/5" aria-expanded={open === i}>
                  {q}
                  <ChevronDown size={20} className={`shrink-0 transition ${open === i ? "rotate-180" : ""}`} />
                </button>
                {open === i && <p className="fade-in px-5 pb-4 text-white/65">{a}</p>}
              </div>
            ))}
          </div>
        </section>

        <section className="mb-8">
          <h2 className="mb-3 text-xl font-bold">Keyboard shortcuts</h2>
          <div className="grid grid-cols-1 gap-x-8 rounded-2xl border border-white/6 bg-panel px-5 py-3 text-sm md:grid-cols-2">
            {[...APP_SHORTCUTS, ...PLAYER_SHORTCUTS].map(([k, v]) => (
              <div key={k + v} className="flex items-center justify-between gap-3 border-b border-white/5 py-2">
                <span className="text-white/70">{v}</span>
                <kbd className="whitespace-nowrap rounded-md bg-white/10 px-2 py-0.5 font-mono text-xs">{k}</kbd>
              </div>
            ))}
          </div>
        </section>

        <Button variant="ghost" onClick={() => navigate("/")}>
          <RotateCcw size={18} /> Back to Home
        </Button>
      </div>
    </div>
  );
}
