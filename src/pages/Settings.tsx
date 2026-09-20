import { useEffect, useState, type ReactNode } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { FolderOpen } from "lucide-react";
import { api, type SystemInfo } from "../lib/api";
import { fmtBytes } from "../lib/format";
import { APP_SHORTCUTS, PLAYER_SHORTCUTS } from "../lib/shortcuts";
import { Button, ConfirmDialog, PageHeader, Toggle } from "../components/ui";
import { checkForUpdatesNow } from "../components/Updater";
import { AddonsSettings } from "../components/AddonsSettings";
import { useApp } from "../store/app";

function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="mb-8 rounded-2xl border border-white/6 bg-panel p-2">
      <h2 className="px-4 pb-1 pt-3 text-sm font-bold uppercase tracking-widest text-white/45">{title}</h2>
      <div className="divide-y divide-white/5">{children}</div>
    </section>
  );
}

function Item({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 px-4 py-4">
      <div className="min-w-0">
        <div className="font-semibold">{label}</div>
        {hint && <div className="text-sm text-white/50">{hint}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

const select = "rounded-lg border border-white/15 bg-panel2 px-3 py-2 text-white outline-none focus:border-white/40";
const LANGS = ["Off", "English", "Hindi", "Spanish", "French", "Arabic", "Portuguese", "Bengali", "Indonesian", "Malay", "Tamil", "Telugu", "Urdu", "Chinese", "Korean", "Japanese", "German", "Italian", "Russian", "Turkish", "Vietnamese", "Thai", "Filipino"];

export function SettingsPage() {
  const settings = useApp((s) => s.settings);
  const save = useApp((s) => s.saveSettings);
  const toast = useApp((s) => s.toast);
  const refreshHistory = useApp((s) => s.refreshHistory);
  const refreshFavorites = useApp((s) => s.refreshFavorites);
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [confirm, setConfirm] = useState<null | "history" | "favorites" | "cache">(null);

  useEffect(() => {
    api.systemInfo().then(setInfo).catch(() => {});
  }, [settings?.downloadDir]);

  if (!settings) return null;

  const browse = async () => {
    const dir = await openDialog({ directory: true, multiple: false, title: "Choose where downloads are saved" });
    if (typeof dir === "string") {
      await save({ downloadDir: dir });
      toast("Download folder changed", "success");
    }
  };

  return (
    <div className="h-full overflow-y-auto px-8 pb-12 pt-8">
      <div className="max-w-3xl">
        <PageHeader title="Settings" subtitle="Changes are saved automatically." />

        <Group title="Playback">
          <Item label="Preferred quality" hint="Auto picks the best picture. Choose lower on slow internet.">
            <select className={select} value={settings.preferredQuality} onChange={(e) => save({ preferredQuality: Number(e.target.value) })}>
              <option value={0}>Auto (Best)</option>
              <option value={1080}>1080p (Full HD)</option>
              <option value={720}>720p (HD)</option>
              <option value={480}>480p (saves data)</option>
              <option value={360}>360p (slow internet)</option>
            </select>
          </Item>
          <Item label="Subtitle language" hint="Used automatically when available.">
            <select className={select} value={settings.subtitleLanguage} onChange={(e) => save({ subtitleLanguage: e.target.value })}>
              {LANGS.map((l) => (
                <option key={l} value={l}>
                  {l}
                </option>
              ))}
            </select>
          </Item>
          <Item label="Play next episode automatically" hint="Starts the next episode after a 10-second countdown.">
            <Toggle label="Autoplay next episode" checked={settings.autoplayNext} onChange={(v) => save({ autoplayNext: v })} />
          </Item>
          <Item label="Remember playback speed" hint="Keep your last speed (like 1.5x) for the next video.">
            <Toggle label="Remember speed" checked={settings.rememberSpeed} onChange={(v) => save({ rememberSpeed: v })} />
          </Item>
          <Item label="Arrow key skip" hint="How far ← and → jump.">
            <select className={select} value={settings.seekStep} onChange={(e) => save({ seekStep: Number(e.target.value) })}>
              {[5, 10, 15, 30, 60].map((s) => (
                <option key={s} value={s}>
                  {s} seconds
                </option>
              ))}
            </select>
          </Item>
          <Item label="Night mode" hint="Evens out loud and quiet parts. Also in the player's audio menu.">
            <Toggle label="Night mode" checked={settings.nightMode} onChange={(v) => save({ nightMode: v })} />
          </Item>
        </Group>

        <Group title="Subtitle style">
          <div className="px-4 py-4">
            <div className="relative mb-4 flex h-32 items-end justify-center overflow-hidden rounded-xl bg-gradient-to-br from-slate-700 via-slate-800 to-slate-900 pb-4">
              <span
                style={{
                  fontSize: Math.round(settings.subtitleSize * 0.5),
                  textShadow: settings.subtitleBackground ? "none" : "0 0 3px #000, 0 0 3px #000, 1px 1px 2px #000",
                  background: settings.subtitleBackground ? "rgba(0,0,0,.6)" : "transparent",
                }}
                className="rounded px-2 font-semibold"
              >
                This is how subtitles will look.
              </span>
            </div>
            <div className="flex items-center justify-between gap-6 py-2">
              <span className="font-semibold">Size</span>
              <input type="range" min={24} max={90} value={settings.subtitleSize} onChange={(e) => save({ subtitleSize: Number(e.target.value) })} className="mb-range w-64" style={{ ["--pct" as string]: `${((settings.subtitleSize - 24) / 66) * 100}%` }} aria-label="Subtitle size" />
            </div>
            <div className="flex items-center justify-between gap-6 py-2">
              <span className="font-semibold">Dark box behind text</span>
              <Toggle label="Subtitle background" checked={settings.subtitleBackground} onChange={(v) => save({ subtitleBackground: v })} />
            </div>
          </div>
        </Group>

        <Group title="Downloads">
          <Item label="Download folder" hint={info ? `${info.downloadDir}${info.freeBytes ? ` · ${fmtBytes(info.freeBytes)} free` : ""}` : ""}>
            <div className="flex gap-2">
              {info && (
                <Button size="sm" variant="ghost" onClick={() => api.openFolder(info.downloadDir)}>
                  <FolderOpen size={16} /> Open
                </Button>
              )}
              <Button size="sm" onClick={browse}>
                Change…
              </Button>
            </div>
          </Item>
          <Item label="Downloads at the same time" hint="More is faster overall on good internet.">
            <select className={select} value={settings.simultaneousDownloads} onChange={(e) => save({ simultaneousDownloads: Number(e.target.value) })}>
              {[1, 2, 3, 4, 5].map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
            </select>
          </Item>
        </Group>

        <Group title="Display">
          <Item label="Text and picture size" hint="Ctrl + and Ctrl − also work anywhere.">
            <div className="flex items-center gap-2">
              <Button size="sm" onClick={() => save({ uiZoom: Math.max(0.6, Math.round((settings.uiZoom - 0.1) * 10) / 10) })}>
                −
              </Button>
              <span className="w-14 text-center tabular-nums">{Math.round(settings.uiZoom * 100)}%</span>
              <Button size="sm" onClick={() => save({ uiZoom: Math.min(2, Math.round((settings.uiZoom + 0.1) * 10) / 10) })}>
                +
              </Button>
            </div>
          </Item>
        </Group>

        <Group title="Library">
          <Item label="Clear watch history" hint="Removes Continue Watching and watched ticks.">
            <Button size="sm" onClick={() => setConfirm("history")}>
              Clear
            </Button>
          </Item>
          <Item label="Clear My List">
            <Button size="sm" onClick={() => setConfirm("favorites")}>
              Clear
            </Button>
          </Item>
          <Item label="Clear cache" hint="Can fix titles or pictures that won't load.">
            <Button size="sm" onClick={() => setConfirm("cache")}>
              Clear
            </Button>
          </Item>
        </Group>

        <Group title="Extra sources">
          <AddonsSettings />
        </Group>

        <Group title="Keyboard shortcuts">
          <div className="grid grid-cols-1 gap-x-8 px-4 py-3 text-sm md:grid-cols-2">
            {[...APP_SHORTCUTS, ...PLAYER_SHORTCUTS].map(([k, v]) => (
              <div key={k + v} className="flex items-center justify-between gap-3 border-b border-white/5 py-2">
                <span className="text-white/70">{v}</span>
                <kbd className="whitespace-nowrap rounded-md bg-white/10 px-2 py-0.5 font-mono text-xs">{k}</kbd>
              </div>
            ))}
          </div>
        </Group>

        <Group title="About">
          <div className="space-y-2 px-4 py-4 text-sm text-white/65">
            <div className="flex flex-wrap items-center gap-3">
              <p className="text-base font-semibold text-white">MovieBox {info?.version}</p>
              <Button size="sm" onClick={() => checkForUpdatesNow(toast)}>
                Check for updates
              </Button>
            </div>
            <p>MovieBox checks for a new version by itself every time you open it.</p>
            <p>A friendly Windows app built on the open-source MovieBox-Tui by mesamirh (MIT / Apache-2.0).</p>
            <p>Video playback by mpv (LGPL) through tauri-plugin-libmpv (MPL-2.0). Built with Tauri, React and lucide icons.</p>
            <p>No accounts, no tracking, no telemetry. Your history and lists stay on this PC.</p>
          </div>
        </Group>
      </div>

      <ConfirmDialog
        open={confirm !== null}
        danger
        title={confirm === "history" ? "Clear watch history?" : confirm === "favorites" ? "Clear My List?" : "Clear cache?"}
        text={confirm === "cache" ? "Saved pictures and lists will be downloaded again." : "This can't be undone."}
        confirmLabel="Clear"
        onClose={() => setConfirm(null)}
        onConfirm={async () => {
          if (confirm === "history") {
            await api.historyClear();
            await refreshHistory();
          } else if (confirm === "favorites") {
            await api.favoritesClear();
            await refreshFavorites();
          } else if (confirm === "cache") {
            await api.clearCache();
          }
          toast("Done", "success");
        }}
      />
    </div>
  );
}
