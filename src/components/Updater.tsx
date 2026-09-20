import { useCallback, useEffect, useRef, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Download, RefreshCw } from "lucide-react";
import { Button, Modal } from "./ui";
import { useApp } from "../store/app";

/** Seconds the "update found" card waits before installing on its own. */
const AUTO_START_AFTER = 6;

type Phase = "idle" | "found" | "downloading" | "installing" | "failed";

/**
 * Checks for a new MovieBox on every launch and installs it.
 * The card counts down first so a restart is never a surprise; "Later" skips
 * this launch. Anything that goes wrong (offline, no release yet) stays quiet —
 * a failed update check must never block watching.
 */
export function Updater() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [update, setUpdate] = useState<Update | null>(null);
  const [pct, setPct] = useState(0);
  const [countdown, setCountdown] = useState(AUTO_START_AFTER);
  const started = useRef(false);
  const toast = useApp((s) => s.toast);

  const install = useCallback(
    async (u: Update) => {
      setPhase("downloading");
      let total = 0;
      let got = 0;
      try {
        await u.downloadAndInstall((e) => {
          if (e.event === "Started") {
            total = e.data.contentLength ?? 0;
          } else if (e.event === "Progress") {
            got += e.data.chunkLength;
            if (total > 0) setPct(Math.min(99, Math.round((got / total) * 100)));
          } else if (e.event === "Finished") {
            setPct(100);
            setPhase("installing");
          }
        });
        await relaunch();
      } catch (err) {
        console.error("update failed", err);
        setPhase("failed");
      }
    },
    [],
  );

  // One check per launch, after the window has settled. The first attempt can land
  // before the machine's connection is ready, so a single retry follows a failure.
  useEffect(() => {
    let cancelled = false;
    const timers: ReturnType<typeof setTimeout>[] = [];
    const attempt = async (retriesLeft: number) => {
      try {
        const u = await check();
        if (u && !cancelled) {
          setUpdate(u);
          setPhase("found");
        }
      } catch (err) {
        console.warn("update check failed", err);
        if (retriesLeft > 0 && !cancelled) timers.push(setTimeout(() => attempt(retriesLeft - 1), 15000));
      }
    };
    timers.push(setTimeout(() => attempt(1), 2500));
    return () => {
      cancelled = true;
      timers.forEach(clearTimeout);
    };
  }, []);

  // countdown, then install by itself
  useEffect(() => {
    if (phase !== "found" || !update) return;
    if (countdown <= 0) {
      if (!started.current) {
        started.current = true;
        install(update);
      }
      return;
    }
    const t = setTimeout(() => setCountdown((c) => c - 1), 1000);
    return () => clearTimeout(t);
  }, [phase, countdown, update, install]);

  if (phase === "idle" || !update) return null;

  if (phase === "failed") {
    return (
      <Modal open onClose={() => setPhase("idle")} title="Update didn't finish" width={430}>
        <p className="text-white/70">
          MovieBox couldn't install the update this time. You can keep watching — it will try again next time you open the app.
        </p>
        <div className="mt-6 flex justify-end">
          <Button variant="primary" onClick={() => setPhase("idle")}>
            OK
          </Button>
        </div>
      </Modal>
    );
  }

  const busy = phase === "downloading" || phase === "installing";

  return (
    <Modal open onClose={() => !busy && setPhase("idle")} title={busy ? "Updating MovieBox" : "A new MovieBox is ready"} width={430}>
      <div className="flex items-start gap-4">
        <div className="mt-1 flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-white/10">
          {busy ? <RefreshCw size={22} className="animate-spin" /> : <Download size={22} />}
        </div>
        <div className="min-w-0 flex-1">
          <p className="text-white/80">
            {busy
              ? phase === "installing"
                ? "Finishing up. MovieBox will restart on its own."
                : "Downloading version " + update.version + "…"
              : `Version ${update.version} is available. MovieBox will update itself and restart in ${countdown}s.`}
          </p>
          {update.body && !busy && <p className="mt-2 whitespace-pre-line text-sm text-white/50">{update.body}</p>}
          {busy && (
            <div className="mt-4 h-2 w-full overflow-hidden rounded-full bg-white/10">
              <div className="h-full rounded-full bg-brand transition-[width]" style={{ width: `${pct}%` }} />
            </div>
          )}
        </div>
      </div>
      {!busy && (
        <div className="mt-6 flex justify-end gap-2">
          <Button
            onClick={() => {
              setPhase("idle");
              toast("MovieBox will update the next time you open it.", "info");
            }}
          >
            Later
          </Button>
          <Button
            variant="primary"
            autoFocus
            onClick={() => {
              started.current = true;
              install(update);
            }}
          >
            Update now
          </Button>
        </div>
      )}
    </Modal>
  );
}

/** Settings → About: "Check for updates". Reports both outcomes, unlike the launch check. */
export async function checkForUpdatesNow(toast: (text: string, kind?: "info" | "success" | "error", action?: { label: string; run: () => void }) => void) {
  try {
    const u = await check();
    if (!u) {
      toast("You're on the latest version of MovieBox.", "success");
      return;
    }
    toast(`Version ${u.version} is available — downloading it now.`, "info");
    await u.downloadAndInstall();
    await relaunch();
  } catch (err) {
    console.error("manual update check failed", err);
    toast("Couldn't check for updates. Please check your internet connection.", "error");
  }
}
