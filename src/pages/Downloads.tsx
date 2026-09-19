import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Download, FolderOpen, Pause, Play, RotateCw, Trash2, X, AlertTriangle, CheckCircle2 } from "lucide-react";
import { api, type DownloadTask } from "../lib/api";
import { epLabel, fmtBytes, fmtEta, fmtSpeed, posterUrl } from "../lib/format";
import { Button, ConfirmDialog, EmptyState, IconButton, PageHeader } from "../components/ui";
import { useApp } from "../store/app";
import { usePlayNow } from "../store/player";

function TaskCard({ t, onAskDelete }: { t: DownloadTask; onAskDelete: (t: DownloadTask) => void }) {
  const playNow = usePlayNow();
  const toast = useApp((s) => s.toast);
  const pct = t.total ? Math.min(100, (t.downloaded / t.total) * 100) : 0;
  const name = t.mediaType === "series" ? `${epLabel(t.season, t.episode)}${t.episodeTitle ? ` · ${t.episodeTitle}` : ""}` : t.year ?? "";
  const statusText =
    t.status === "downloading"
      ? [fmtSpeed(t.speed), t.total ? fmtEta(t.total - t.downloaded, t.speed) : ""].filter(Boolean).join(" · ") || "Starting…"
      : t.status === "queued"
        ? "Waiting to start"
        : t.status === "paused"
          ? "Paused"
          : t.status === "failed"
            ? t.error ?? "Download failed"
            : [fmtBytes(t.total ?? t.downloaded), t.height ? `${t.height}p` : ""].filter(Boolean).join(" · ");

  return (
    <div className="fade-in flex items-center gap-4 rounded-2xl border border-white/6 bg-panel p-3 pr-4">
      {t.poster ? <img src={posterUrl(t.poster, 185)} alt="" className="h-24 w-16 shrink-0 rounded-lg object-cover" /> : <div className="h-24 w-16 shrink-0 rounded-lg bg-panel2" />}
      <div className="min-w-0 flex-1">
        <div className="truncate text-lg font-semibold">{t.title}</div>
        <div className="truncate text-sm text-white/55">{name}</div>
        {t.status !== "done" && (
          <div className="mt-2 flex items-center gap-3">
            <div className="h-2 flex-1 overflow-hidden rounded-full bg-white/10">
              <div className={`h-full transition-all ${t.status === "failed" ? "bg-yellow-500" : t.status === "paused" ? "bg-white/40" : "bg-brand"}`} style={{ width: `${pct}%` }} />
            </div>
            <span className="w-24 text-right text-sm tabular-nums text-white/60">
              {fmtBytes(t.downloaded)}
              {t.total ? ` / ${fmtBytes(t.total)}` : ""}
            </span>
          </div>
        )}
        <div className={`mt-1 flex items-center gap-1.5 text-sm ${t.status === "failed" ? "text-yellow-400" : t.status === "done" ? "text-green-400" : "text-white/55"}`}>
          {t.status === "failed" && <AlertTriangle size={14} />}
          {t.status === "done" && <CheckCircle2 size={14} />}
          {statusText}
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-1">
        {t.status === "done" ? (
          <>
            <Button
              variant="primary"
              size="sm"
              onClick={() =>
                playNow({
                  ref: { id: t.subjectId, title: t.title, poster: t.poster, mediaType: t.mediaType, year: t.year, season: t.season, episode: t.episode },
                  seasons: [],
                  dubIds: [],
                  episodeTitle: t.episodeTitle,
                  startAt: null,
                  localPath: t.path,
                })
              }
            >
              <Play size={16} fill="currentColor" /> Play
            </Button>
            <IconButton title="Open folder" onClick={() => api.openFolder(t.path).catch(() => toast("Couldn't open the folder.", "error"))}>
              <FolderOpen size={20} />
            </IconButton>
            <IconButton title="Delete" onClick={() => onAskDelete(t)}>
              <Trash2 size={20} />
            </IconButton>
          </>
        ) : (
          <>
            {(t.status === "downloading" || t.status === "queued") && (
              <IconButton title="Pause" onClick={() => api.downloadPause(t.id)}>
                <Pause size={20} />
              </IconButton>
            )}
            {t.status === "paused" && (
              <IconButton title="Resume" onClick={() => api.downloadResume(t.id)}>
                <Play size={20} />
              </IconButton>
            )}
            {t.status === "failed" && (
              <IconButton title="Try again" onClick={() => api.downloadResume(t.id)}>
                <RotateCw size={20} />
              </IconButton>
            )}
            <IconButton title="Cancel download" onClick={() => onAskDelete(t)}>
              <X size={20} />
            </IconButton>
          </>
        )}
      </div>
    </div>
  );
}

export function DownloadsPage() {
  const downloads = useApp((s) => s.downloads);
  const removeDownload = useApp((s) => s.removeDownload);
  const toast = useApp((s) => s.toast);
  const [tab, setTab] = useState<"active" | "done">("active");
  const [ask, setAsk] = useState<DownloadTask | null>(null);
  const navigate = useNavigate();

  const active = useMemo(() => downloads.filter((d) => d.status !== "done").sort((a, b) => a.added - b.added), [downloads]);
  const done = useMemo(() => downloads.filter((d) => d.status === "done").sort((a, b) => b.added - a.added), [downloads]);
  const list = tab === "active" ? active : done;
  const pausable = active.filter((d) => d.status === "downloading" || d.status === "queued");

  return (
    <div className="h-full overflow-y-auto px-8 pb-10 pt-8">
      <PageHeader
        title="Downloads"
        subtitle="Downloaded videos play without internet."
        right={
          tab === "active" && active.length > 1 ? (
            <div className="flex gap-2">
              {pausable.length > 0 ? (
                <Button size="sm" onClick={() => pausable.forEach((d) => api.downloadPause(d.id))}>
                  <Pause size={16} /> Pause all
                </Button>
              ) : (
                <Button size="sm" onClick={() => active.forEach((d) => api.downloadResume(d.id))}>
                  <Play size={16} /> Resume all
                </Button>
              )}
            </div>
          ) : undefined
        }
      />
      <div className="mb-5 flex gap-2">
        {([["active", `Downloading (${active.length})`], ["done", `Finished (${done.length})`]] as const).map(([k, label]) => (
          <button key={k} onClick={() => setTab(k)} className={`rounded-full px-5 py-2 text-sm font-semibold transition ${tab === k ? "bg-white text-black" : "bg-white/10 text-white/80 hover:bg-white/20"}`}>
            {label}
          </button>
        ))}
      </div>
      {list.length === 0 ? (
        <EmptyState
          icon={<Download size={36} />}
          title={tab === "active" ? "Nothing downloading" : "No finished downloads yet"}
          text="Open any movie or series and press Download. You can watch downloads without internet."
          action={<Button variant="primary" onClick={() => navigate("/")}>Browse titles</Button>}
        />
      ) : (
        <div className="flex max-w-4xl flex-col gap-3">
          {list.map((t) => (
            <TaskCard key={t.id} t={t} onAskDelete={setAsk} />
          ))}
        </div>
      )}
      <ConfirmDialog
        open={!!ask}
        danger
        title={ask?.status === "done" ? "Delete this download?" : "Cancel this download?"}
        text={ask?.status === "done" ? "The video file will be removed from your PC." : "The partly downloaded file will be removed."}
        confirmLabel={ask?.status === "done" ? "Delete" : "Cancel download"}
        cancelLabel="Keep"
        onClose={() => setAsk(null)}
        onConfirm={() => {
          if (!ask) return;
          api.downloadRemove(ask.id, true).then(() => {
            removeDownload(ask.id);
            toast(ask.status === "done" ? "Download deleted" : "Download cancelled", "success");
          });
        }}
      />
    </div>
  );
}
