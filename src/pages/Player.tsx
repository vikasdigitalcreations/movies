import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { getCurrentWindow, currentMonitor } from "@tauri-apps/api/window";
import { LogicalPosition, LogicalSize, type PhysicalPosition, type PhysicalSize } from "@tauri-apps/api/dpi";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  ArrowLeft,
  Captions,
  Check,
  ChevronRight,
  Gauge,
  Info,
  ListVideo,
  Maximize,
  Minimize,
  Moon,
  MoonStar,
  Pause,
  PictureInPicture2,
  Pin,
  Play,
  RotateCcw,
  RotateCw,
  Settings2,
  SkipBack,
  SkipForward,
  Timer,
  Volume1,
  Volume2,
  VolumeX,
  AudioLines,
  Keyboard,
  Loader2,
  X,
} from "lucide-react";
import { api, errText, type Stream, type Subtitle } from "../lib/api";
import { load, mpv, onEvents, onProps, playerInit, type Track } from "../lib/player";
import { absIndex, clamp, epLabel, fmtBytes, fmtTime } from "../lib/format";
import { PLAYER_SHORTCUTS } from "../lib/shortcuts";
import { neighbor, usePlayer, type Session } from "../store/player";
import { useApp } from "../store/app";

const SPEEDS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4, 5, 6, 8];
const ASPECTS = ["Fit", "Fill", "Stretch"] as const;
type Menu = null | "speed" | "subs" | "audio" | "quality" | "sleep";

function pickIndex(list: Stream[], pref: number) {
  if (!pref) return 0;
  let best = -1;
  list.forEach((s, i) => {
    if (!s.multi && s.height <= pref && (best < 0 || s.height > list[best].height)) best = i;
  });
  if (best >= 0) return best;
  const m = list.findIndex((s) => s.multi);
  return m >= 0 ? m : list.length - 1;
}

function MenuPanel({ title, children, onClose }: { title: string; children: ReactNode; onClose: () => void }) {
  return (
    <div className="fade-in absolute bottom-24 right-6 z-40 max-h-[60vh] w-72 overflow-y-auto rounded-2xl border border-white/10 bg-black/90 p-2 shadow-2xl backdrop-blur" onMouseDown={(e) => e.stopPropagation()} onClick={(e) => e.stopPropagation()}>
      <div className="flex items-center justify-between px-3 py-2">
        <span className="font-bold">{title}</span>
        <button className="rounded-full p-1 text-white/60 hover:bg-white/10" onClick={onClose} aria-label="Close menu">
          <X size={16} />
        </button>
      </div>
      {children}
    </div>
  );
}

function MenuItem({ active, onClick, children, sub }: { active?: boolean; onClick: () => void; children: ReactNode; sub?: string }) {
  return (
    <button onClick={onClick} className={`flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left hover:bg-white/10 ${active ? "text-white" : "text-white/80"}`}>
      <span className="w-4">{active && <Check size={16} />}</span>
      <span className="flex-1 truncate">{children}</span>
      {sub && <span className="text-xs text-white/45">{sub}</span>}
    </button>
  );
}

export function PlayerPage() {
  const navigate = useNavigate();
  const session = usePlayer((s) => s.session);
  const startSession = usePlayer((s) => s.start);
  const settings = useApp((s) => s.settings);
  const saveSettings = useApp((s) => s.saveSettings);
  const toast = useApp((s) => s.toast);
  const refreshHistory = useApp((s) => s.refreshHistory);

  const [phase, setPhase] = useState<"resolving" | "resume" | "loading" | "playing" | "error">("resolving");
  const [streams, setStreams] = useState<Stream[]>([]);
  const [current, setCurrent] = useState<Stream | null>(null);
  const [subs, setSubs] = useState<Subtitle[]>([]);
  const [activeOnlineSub, setActiveOnlineSub] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState("");
  const [resumeAt, setResumeAt] = useState(0);

  const [paused, setPaused] = useState(false);
  const [pos, setPos] = useState(0);
  const [duration, setDuration] = useState(0);
  const [volume, setVolume] = useState(settings?.volume ?? 100);
  const [muted, setMuted] = useState(false);
  const [speed, setSpeed] = useState(1);
  const [buffering, setBuffering] = useState(false);
  const [bufPct, setBufPct] = useState<number | null>(null);
  const [cacheTime, setCacheTime] = useState(0);
  const [cacheSpeed, setCacheSpeed] = useState(0);
  const [tracks, setTracks] = useState<Track[]>([]);
  const [chapters, setChapters] = useState<{ time: number; title?: string }[]>([]);
  const [videoParams, setVideoParams] = useState<{ w?: number; h?: number } | null>(null);
  const [subDelay, setSubDelay] = useState(0);

  const [controls, setControls] = useState(true);
  const [menu, setMenu] = useState<Menu>(null);
  const [osd, setOsd] = useState<string | null>(null);
  const [remaining, setRemaining] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  const [onTop, setOnTop] = useState(false);
  const [mini, setMini] = useState(false);
  const [showKeys, setShowKeys] = useState(false);
  const [showInfo, setShowInfo] = useState(false);
  const [drawer, setDrawer] = useState(false);
  const [upNext, setUpNext] = useState<number | null>(null);
  const [upNextDismissed, setUpNextDismissed] = useState(false);
  const [stallOffer, setStallOffer] = useState(false);
  const [sleep, setSleep] = useState<{ kind: "time"; at: number; minutes: number } | { kind: "episode" } | null>(null);
  const [sleepDone, setSleepDone] = useState(false);
  const [night, setNight] = useState(settings?.nightMode ?? false);
  const [aspect, setAspect] = useState(0);
  const [hoverX, setHoverX] = useState<number | null>(null);
  const [subSize, setSubSize] = useState(settings?.subtitleSize ?? 46);

  const streamsRef = useRef<Stream[]>([]);
  const attempt = useRef({ index: 0, triedAlt: false, started: false, token: 0 });
  const live = useRef({ pos: 0, duration: 0, paused: false, volume: 100, speed: 1 });
  const hideTimer = useRef<number | undefined>(undefined);
  const osdTimer = useRef<number | undefined>(undefined);
  const stalls = useRef<number[]>([]);
  const prevWin = useRef<{ size: PhysicalSize; pos: PhysicalPosition; max: boolean } | null>(null);
  const clickTimer = useRef<number | undefined>(undefined);
  const seekRef = useRef<HTMLDivElement>(null);

  live.current = { pos, duration, paused, volume, speed };
  const ref = session?.ref;
  const isSeries = ref?.mediaType === "series" && !!session?.seasons.length;
  const next = session && isSeries ? neighbor(session.seasons, ref!.season, ref!.episode, 1) : null;
  const prev = session && isSeries ? neighbor(session.seasons, ref!.season, ref!.episode, -1) : null;
  const pref = session?.qualityOverride ?? settings?.preferredQuality ?? 0;

  const flash = useCallback((text: string) => {
    setOsd(text);
    window.clearTimeout(osdTimer.current);
    osdTimer.current = window.setTimeout(() => setOsd(null), 1200);
  }, []);

  const poke = useCallback(() => {
    setControls(true);
    window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => {
      if (!live.current.paused) setControls(false);
    }, 3000);
  }, []);

  // ---- body/window setup -------------------------------------------------
  useEffect(() => {
    document.body.classList.add("playing");
    api.keepAwake(true, true).catch(() => {});
    poke();
    return () => {
      document.body.classList.remove("playing");
      api.keepAwake(false, false).catch(() => {});
      mpv.stop().catch(() => {});
      const w = getCurrentWindow();
      w.setFullscreen(false).catch(() => {});
      w.setAlwaysOnTop(false).catch(() => {});
      if (prevWin.current) {
        const p = prevWin.current;
        w.setMinSize(new LogicalSize(900, 600)).catch(() => {});
        w.setSize(p.size).then(() => w.setPosition(p.pos)).catch(() => {});
        if (p.max) w.maximize().catch(() => {});
      }
      refreshHistory();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!session) navigate("/", { replace: true });
  }, [session, navigate]);

  // ---- mpv observers -----------------------------------------------------
  useEffect(() => {
    const unProps = onProps(({ name, data }) => {
      const v = data as unknown;
      switch (name) {
        case "pause":
          setPaused(!!v);
          if (v) setControls(true);
          break;
        case "time-pos":
          if (typeof v === "number") setPos(v);
          break;
        case "duration":
          setDuration(typeof v === "number" ? v : 0);
          break;
        case "volume":
          if (typeof v === "number") setVolume(v);
          break;
        case "mute":
          setMuted(!!v);
          break;
        case "speed":
          if (typeof v === "number") setSpeed(v);
          break;
        case "paused-for-cache":
          setBuffering(!!v);
          if (v && attempt.current.started) {
            const now = Date.now();
            stalls.current = [...stalls.current.filter((t) => now - t < 60000), now];
            if (stalls.current.length >= 3) setStallOffer(true);
          }
          break;
        case "cache-buffering-state":
          setBufPct(typeof v === "number" ? v : null);
          break;
        case "demuxer-cache-time":
          setCacheTime(typeof v === "number" ? v : 0);
          break;
        case "cache-speed":
          setCacheSpeed(typeof v === "number" ? v : 0);
          break;
        case "track-list":
          setTracks(Array.isArray(v) ? (v as Track[]) : []);
          break;
        case "chapter-list":
          setChapters(Array.isArray(v) ? (v as { time: number; title?: string }[]) : []);
          break;
        case "video-params":
          setVideoParams(v && typeof v === "object" ? { w: (v as Record<string, number>).w, h: (v as Record<string, number>).h } : null);
          break;
        case "sub-delay":
          if (typeof v === "number") setSubDelay(v);
          break;
        case "eof-reached":
          if (v === true) endedRef.current();
          break;
      }
    });
    const unEvents = onEvents((ev) => {
      const e = ev as unknown as { event: string; reason?: string; file_error?: string };
      if (e.event === "file-loaded") {
        attempt.current.started = true;
        setPhase("playing");
      } else if (e.event === "end-file" && e.reason === "error") {
        failRef.current();
      } else if (e.event === "end-file" && e.reason === "eof") {
        endedRef.current();
      }
    });
    return () => {
      unProps.then((u) => u());
      unEvents.then((u) => u());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ---- loading -----------------------------------------------------------
  const startStream = useCallback(
    async (list: Stream[], index: number, startPos: number) => {
      const s = list[index];
      if (!s) return;
      const token = ++attempt.current.token;
      attempt.current.index = index;
      attempt.current.started = false;
      stalls.current = [];
      setStallOffer(false);
      setCurrent(s);
      setPhase("loading");
      try {
        await load(s.url, s.headers, startPos);
        if (settings?.rememberSpeed && settings.lastSpeed && settings.lastSpeed !== 1) mpv.set("speed", settings.lastSpeed).catch(() => {});
      } catch (e) {
        if (token === attempt.current.token) failRef.current(errText(e));
      }
      window.setTimeout(() => {
        if (token === attempt.current.token && !attempt.current.started) failRef.current("timeout");
      }, 60000);
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [settings],
  );

  const handleFailure = async (_why?: string) => {
    if (!session) return;
    const a = attempt.current;
    const at = live.current.pos > 1 ? live.current.pos : resumeAtRef.current;
    const list = streamsRef.current;
    if (a.index + 1 < list.length) {
      toast("This source isn't working, trying another…");
      startStream(list, a.index + 1, at);
      return;
    }
    if (!a.triedAlt && !session.localPath) {
      a.triedAlt = true;
      toast("Trying another source…");
      try {
        const alt = await api.alternateSource(session.ref.title, session.ref.year, session.ref.season, session.ref.episode, pref);
        const list2 = [...streamsRef.current, alt];
        streamsRef.current = list2;
        setStreams(list2);
        startStream(list2, list2.length - 1, at);
        return;
      } catch {
        /* fall through */
      }
    }
    setErrorMsg(session.localPath ? "This downloaded file can't be played. It may have been moved or deleted." : "Sorry, this video isn't working right now. Please try again in a little while.");
    setPhase("error");
  };

  const resumeAtRef = useRef(0);

  const begin = useCallback(
    async (list: Stream[], startPos: number) => {
      if (!session) return;
      resumeAtRef.current = startPos;
      await mpv.set("sub-font-size", settings?.subtitleSize ?? 46).catch(() => {});
      await mpv.set("sub-border-style", settings?.subtitleBackground ? "background-box" : "outline-and-shadow").catch(() => {});
      await mpv.set("sub-back-color", settings?.subtitleBackground ? "#99000000" : "#00000000").catch(() => {});
      await mpv.set("af", night ? "lavfi=[loudnorm=I=-16:LRA=11:TP=-1.5]" : "").catch(() => {});
      api.historyStart(session.ref, startPos).catch(() => {});
      startStream(list, pickIndex(list, session.localPath ? 0 : pref), startPos);
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [session, settings, night, pref, startStream],
  );

  useEffect(() => {
    if (!session || !settings) return;
    let cancelled = false;
    (async () => {
      setPhase("resolving");
      setSubs([]);
      setActiveOnlineSub(null);
      setUpNext(null);
      setUpNextDismissed(false);
      setSleepDone(false);
      setPos(0);
      setDuration(0);
      attempt.current = { index: 0, triedAlt: false, started: false, token: attempt.current.token + 1 };
      try {
        await playerInit({ volume: settings.volume, subSize: settings.subtitleSize, subBackground: settings.subtitleBackground });
      } catch (e) {
        setErrorMsg("The video player couldn't start: " + errText(e));
        setPhase("error");
        return;
      }
      await mpv.stop().catch(() => {});
      const r = session.ref;
      let list: Stream[];
      if (session.localPath) {
        list = [{ label: "Downloaded", height: 0, multi: false, url: session.localPath, headers: [], downloadable: false, source: "This PC" }];
      } else {
        try {
          list = await api.streams(r.id, r.season, r.episode, absIndex(session.seasons, r.season, r.episode));
        } catch (e) {
          if (cancelled) return;
          streamsRef.current = [];
          attempt.current.index = 0;
          // no MovieBox streams: try the alternate source directly
          try {
            const alt = await api.alternateSource(r.title, r.year, r.season, r.episode, pref);
            list = [alt];
            attempt.current.triedAlt = true;
          } catch {
            setErrorMsg(errText(e));
            setPhase("error");
            return;
          }
        }
      }
      if (cancelled) return;
      streamsRef.current = list;
      setStreams(list);
      let start = session.startAt ?? 0;
      if (session.startAt == null) {
        const h = await api.historyGet(r.id, r.season, r.episode).catch(() => null);
        if (h && !h.completed && h.progress > 30 && (!h.duration || h.progress < h.duration - 30)) {
          if (cancelled) return;
          setResumeAt(h.progress);
          setPhase("resume");
          return;
        }
        start = 0;
      }
      if (!cancelled) begin(list, start);
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session]);

  // ---- subtitles ---------------------------------------------------------
  useEffect(() => {
    if (phase !== "playing" || !session || !current) return;
    if (session.localPath) {
      if (settings?.subtitleLanguage === "Off") mpv.set("sid", "no").catch(() => {});
      return;
    }
    if (!current.resourceId || subs.length) return;
    let cancelled = false;
    api
      .subtitles(session.ref.id, current.resourceId, session.dubIds, session.ref.season, session.ref.episode)
      .then(async (list) => {
        if (cancelled) return;
        setSubs(list);
        const lang = settings?.subtitleLanguage ?? "Off";
        if (lang !== "Off" && !activeOnlineSub) {
          const m = list.find((s) => s.name.toLowerCase().includes(lang.toLowerCase()));
          if (m) selectOnlineSub(m, true);
        }
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [phase, current]);

  const selectOnlineSub = async (s: Subtitle, quiet = false) => {
    try {
      const path = await api.fetchSubtitle(s.url);
      await mpv.addSub(path, s.name);
      setActiveOnlineSub(s.url);
      if (!quiet) flash(`Subtitles: ${s.name}`);
    } catch {
      toast("Couldn't load those subtitles. Try another language.", "error");
    }
  };

  // ---- progress saving ---------------------------------------------------
  const save = useCallback(async () => {
    if (!session) return;
    const { pos: p, duration: d } = live.current;
    if (p < 5) return;
    try {
      await api.historyProgress(session.ref, p, d || null);
    } catch {
      /* ignore */
    }
  }, [session]);

  useEffect(() => {
    if (phase !== "playing") return;
    const t = window.setInterval(save, 10000);
    return () => {
      window.clearInterval(t);
      save();
    };
  }, [phase, save]);

  // ---- end of episode / up next / sleep timer ---------------------------
  const goTo = useCallback(
    (target: { season: number; episode: number; title?: string | null } | null, startAt: number | null = 0) => {
      if (!session || !target) return;
      save();
      const s: Session = { ...session, ref: { ...session.ref, season: target.season, episode: target.episode }, episodeTitle: target.title, startAt, localPath: undefined };
      startSession(s);
      flash(epLabel(target.season, target.episode));
    },
    [session, save, startSession, flash],
  );

  function onEnded() {
    if (endedToken.current === attempt.current.token) return;
    endedToken.current = attempt.current.token;
    const L = live.current;
    if (sleepRef.current?.kind === "episode") {
      mpv.set("pause", true).catch(() => {});
      setUpNext(null);
      setUpNextDismissed(true);
      setSleep(null);
      setSleepDone(true);
      return;
    }
    if (nextRef.current && autoplayRef.current && !dismissRef.current && !sleepDoneRef.current) {
      goToRef.current(nextRef.current, 0);
    } else if (L.duration > 0) {
      setControls(true);
    }
  }
  const sleepRef = useRef(sleep);
  sleepRef.current = sleep;
  const nextRef = useRef(next);
  nextRef.current = next;
  const autoplayRef = useRef(settings?.autoplayNext ?? true);
  autoplayRef.current = settings?.autoplayNext ?? true;
  const dismissRef = useRef(upNextDismissed);
  dismissRef.current = upNextDismissed;
  const sleepDoneRef = useRef(sleepDone);
  sleepDoneRef.current = sleepDone;
  const goToRef = useRef(goTo);
  goToRef.current = goTo;
  const endedToken = useRef(-1);
  const endedRef = useRef(onEnded);
  endedRef.current = onEnded;
  const failRef = useRef(handleFailure);
  failRef.current = handleFailure;

  useEffect(() => {
    if (phase !== "playing" || !next || !settings?.autoplayNext || upNextDismissed || sleepDone || sleep?.kind === "episode") return;
    if (duration > 60 && duration - pos <= 12 && upNext === null && !paused) setUpNext(10);
  }, [pos, duration, phase, next, settings, upNextDismissed, upNext, paused, sleep, sleepDone]);

  useEffect(() => {
    if (upNext === null) return;
    if (upNext <= 0) {
      setUpNext(null);
      goTo(next, 0);
      return;
    }
    const t = window.setTimeout(() => setUpNext((x) => (x === null ? null : x - 1)), 1000);
    return () => window.clearTimeout(t);
  }, [upNext, goTo, next]);

  useEffect(() => {
    if (!sleep || sleep.kind !== "time") return;
    const t = window.setInterval(() => {
      if (Date.now() >= sleep.at) {
        mpv.set("pause", true).catch(() => {});
        setUpNext(null);
        setUpNextDismissed(true);
        setSleep(null);
        setSleepDone(true);
      }
    }, 1000);
    return () => window.clearInterval(t);
  }, [sleep]);

  // ---- actions -----------------------------------------------------------
  const togglePause = () => {
    mpv.set("pause", !live.current.paused).catch(() => {});
    flash(live.current.paused ? "Play" : "Pause");
  };
  const seekBy = (s: number) => {
    mpv.seek(s).catch(() => {});
    flash(`${s > 0 ? "+" : "−"}${Math.abs(s) >= 60 ? `${Math.abs(s) / 60} min` : `${Math.abs(s)} s`}`);
  };
  const setVol = (v: number) => {
    const nv = clamp(Math.round(v), 0, 150);
    mpv.set("volume", nv).catch(() => {});
    if (muted) mpv.set("mute", false).catch(() => {});
    flash(`Volume ${nv}%`);
    saveVolume(nv);
  };
  const volTimer = useRef<number | undefined>(undefined);
  const saveVolume = (v: number) => {
    window.clearTimeout(volTimer.current);
    volTimer.current = window.setTimeout(() => saveSettings({ volume: v }), 800);
  };
  const setSpd = (v: number) => {
    mpv.set("speed", v).catch(() => {});
    flash(`Speed ${v}x`);
    if (settings?.rememberSpeed) saveSettings({ lastSpeed: v });
  };
  const stepSpeed = (dir: 1 | -1) => {
    const cur = live.current.speed;
    let i = SPEEDS.findIndex((s) => Math.abs(s - cur) < 0.001);
    if (i < 0) i = SPEEDS.findIndex((s) => s > cur) - (dir > 0 ? 1 : 0);
    setSpd(SPEEDS[clamp(i + dir, 0, SPEEDS.length - 1)]);
  };
  const toggleFs = async () => {
    const w = getCurrentWindow();
    const fs = !(await w.isFullscreen());
    await w.setFullscreen(fs);
    setFullscreen(fs);
  };
  const toggleTop = async () => {
    const v = !onTop;
    await getCurrentWindow().setAlwaysOnTop(v);
    setOnTop(v);
    flash(v ? "Always on top: on" : "Always on top: off");
  };
  const toggleMini = async () => {
    const w = getCurrentWindow();
    if (!mini) {
      prevWin.current = { size: await w.outerSize(), pos: await w.outerPosition(), max: await w.isMaximized() };
      if (await w.isFullscreen()) {
        await w.setFullscreen(false);
        setFullscreen(false);
      }
      if (prevWin.current.max) await w.unmaximize();
      await w.setMinSize(new LogicalSize(320, 180));
      await w.setSize(new LogicalSize(520, 300));
      const mon = await currentMonitor();
      if (mon) {
        const sf = mon.scaleFactor;
        await w.setPosition(new LogicalPosition(mon.position.x / sf + mon.size.width / sf - 540, mon.position.y / sf + mon.size.height / sf - 360));
      }
      await w.setAlwaysOnTop(true);
      setMini(true);
      setMenu(null);
      setDrawer(false);
    } else {
      const p = prevWin.current;
      await w.setAlwaysOnTop(onTop);
      await w.setMinSize(new LogicalSize(900, 600));
      if (p) {
        await w.setSize(p.size);
        await w.setPosition(p.pos);
        if (p.max) await w.maximize();
      }
      prevWin.current = null;
      setMini(false);
    }
  };
  const cycleSub = async () => {
    const subTracks = tracks.filter((t) => t.type === "sub");
    if (!subTracks.length) {
      flash("No subtitles");
      return;
    }
    const cur = subTracks.findIndex((t) => t.selected);
    const nxt = cur + 1 >= subTracks.length ? null : subTracks[cur + 1];
    await mpv.set("sid", nxt ? nxt.id : "no").catch(() => {});
    flash(nxt ? `Subtitles: ${nxt.title || nxt.lang || `Track ${nxt.id}`}` : "Subtitles off");
  };
  const cycleAudio = async () => {
    const a = tracks.filter((t) => t.type === "audio");
    if (a.length < 2) {
      flash(a.length ? `Audio: ${a[0].title || a[0].lang || "Track 1"}` : "No other audio");
      return;
    }
    const cur = a.findIndex((t) => t.selected);
    const nxt = a[(cur + 1) % a.length];
    await mpv.set("aid", nxt.id).catch(() => {});
    flash(`Audio: ${nxt.title || nxt.lang || `Track ${nxt.id}`}`);
  };
  const setAspectMode = (i: number) => {
    setAspect(i);
    if (i === 0) {
      mpv.set("keepaspect", true).catch(() => {});
      mpv.set("panscan", 0).catch(() => {});
    } else if (i === 1) {
      mpv.set("keepaspect", true).catch(() => {});
      mpv.set("panscan", 1).catch(() => {});
    } else {
      mpv.set("panscan", 0).catch(() => {});
      mpv.set("keepaspect", false).catch(() => {});
    }
    flash(`Picture: ${ASPECTS[i]}`);
  };
  const changeSubSize = (delta: number) => {
    const v = clamp(subSize + delta, 20, 100);
    setSubSize(v);
    mpv.set("sub-font-size", v).catch(() => {});
    flash(`Subtitle size ${v}`);
    saveSettings({ subtitleSize: v });
  };
  const toggleNight = () => {
    const v = !night;
    setNight(v);
    mpv.set("af", v ? "lavfi=[loudnorm=I=-16:LRA=11:TP=-1.5]" : "").catch(() => {});
    flash(v ? "Night mode on: quiet and loud parts evened out" : "Night mode off");
    saveSettings({ nightMode: v });
  };
  const screenshot = async () => {
    try {
      const info = await api.systemInfo();
      const name = `${(session?.ref.title ?? "MovieBox").replace(/[\\/:*?"<>|]/g, "_")} ${fmtTime(live.current.pos).replace(/:/g, "-")} ${Date.now() % 100000}.png`;
      const path = `${info.screenshotDir}\\${name}`;
      await mpv.screenshot(path);
      toast("Screenshot saved to Pictures\\MovieBox", "success", { label: "Open folder", run: () => api.openFolder(path) });
    } catch {
      toast("Couldn't save the screenshot.", "error");
    }
  };
  const switchQuality = (i: number) => {
    setMenu(null);
    flash(`Quality: ${streams[i].label}`);
    startStream(streamsRef.current, i, live.current.pos);
  };
  const loadSubFile = async () => {
    setMenu(null);
    const file = await openDialog({ multiple: false, filters: [{ name: "Subtitles", extensions: ["srt", "vtt", "ass", "ssa", "sub"] }] });
    if (typeof file === "string") {
      await mpv.addSub(file, file.split(/[\\/]/).pop() ?? "Subtitles").catch(() => toast("Couldn't load that subtitle file.", "error"));
      flash("Subtitles loaded");
    }
  };
  const leave = () => {
    save();
    navigate(-1);
  };

  // ---- keyboard ----------------------------------------------------------
  const keyRef = useRef<(e: KeyboardEvent) => void>(() => {});
  keyRef.current = (e: KeyboardEvent) => {
    const tag = (e.target as HTMLElement | null)?.tagName;
    if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA") return;
    const k = e.key;
    const lower = k.length === 1 ? k.toLowerCase() : k;
    let handled = true;
    if (phase === "resume") {
      if (k === "Enter") chooseResume(true);
      else if (k === "Escape") leave();
      else handled = false;
    } else if (k === " " || (lower === "k" && !e.ctrlKey)) togglePause();
    else if (lower === "f" && !e.ctrlKey) toggleFs();
    else if (k === "Escape") {
      if (menu || showKeys || showInfo || drawer) {
        setMenu(null);
        setShowKeys(false);
        setShowInfo(false);
        setDrawer(false);
      } else if (fullscreen) toggleFs();
      else if (mini) toggleMini();
      else handled = false;
    } else if (k === "ArrowUp" && e.shiftKey) changeSubSize(4);
    else if (k === "ArrowDown" && e.shiftKey) changeSubSize(-4);
    else if (k === "ArrowUp") setVol(live.current.volume + 5);
    else if (k === "ArrowDown") setVol(live.current.volume - 5);
    else if (k === "ArrowLeft" || k === "ArrowRight") {
      const step = e.ctrlKey ? 60 : e.altKey ? 10 : e.shiftKey ? 5 : settings?.seekStep ?? 10;
      seekBy(k === "ArrowLeft" ? -step : step);
    } else if (lower === "m" && e.shiftKey) toggleMini();
    else if (lower === "m") {
      mpv.set("mute", !muted).catch(() => {});
      flash(muted ? "Sound on" : "Muted");
    } else if (lower === "s" && e.ctrlKey) screenshot();
    else if ((lower === "c" || lower === "s") && !e.ctrlKey) cycleSub();
    else if (lower === "z") {
      mpv.set("sub-delay", Math.round((subDelay - 0.1) * 10) / 10).catch(() => {});
      flash(`Subtitle delay ${(subDelay - 0.1).toFixed(1)} s`);
    } else if (lower === "x") {
      mpv.set("sub-delay", Math.round((subDelay + 0.1) * 10) / 10).catch(() => {});
      flash(`Subtitle delay ${(subDelay + 0.1).toFixed(1)} s`);
    } else if (lower === "a" && !e.ctrlKey) cycleAudio();
    else if (lower === "n" && !e.ctrlKey) {
      if (next) goTo(next, 0);
      else flash("This is the last episode");
    } else if (lower === "p" && !e.ctrlKey) {
      if (prev) goTo(prev, 0);
      else flash("This is the first episode");
    } else if (lower === "w") setAspectMode((aspect + 1) % 3);
    else if (lower === "t") toggleTop();
    else if (lower === "i") setShowInfo((v) => !v);
    else if (lower === "e") {
      if (isSeries) setDrawer((v) => !v);
      else flash("Episode list is for series");
    } else if (k === "?") setShowKeys((v) => !v);
    else if (k === "[") stepSpeed(-1);
    else if (k === "]") stepSpeed(1);
    else if (k === "Backspace") setSpd(1);
    else if (/^[0-9]$/.test(k) && !e.ctrlKey && !e.altKey) {
      mpv.seek(Number(k) * 10, "absolute-percent").catch(() => {});
      flash(`${Number(k) * 10}%`);
    } else if (k === "Home") mpv.seek(0, "absolute").catch(() => {});
    else if (k === "End") mpv.seek(Math.max(0, live.current.duration - 2), "absolute").catch(() => {});
    else if (k === ",") mpv.frameBackStep().catch(() => {});
    else if (k === ".") mpv.frameStep().catch(() => {});
    else handled = false;
    if (handled) {
      e.preventDefault();
      e.stopPropagation();
      poke();
    }
  };

  useEffect(() => {
    const h = (e: KeyboardEvent) => keyRef.current(e);
    window.addEventListener("keydown", h, true);
    return () => window.removeEventListener("keydown", h, true);
  }, []);

  const chooseResume = (resume: boolean) => {
    const start = resume ? Math.max(0, resumeAt - 5) : 0;
    begin(streamsRef.current, start);
  };

  // ---- render ------------------------------------------------------------
  const subTracks = tracks.filter((t) => t.type === "sub");
  const audioTracks = tracks.filter((t) => t.type === "audio");
  const pct = duration > 0 ? (pos / duration) * 100 : 0;
  const bufferedPct = duration > 0 ? Math.min(100, (cacheTime / duration) * 100) : 0;
  const showControls = controls || paused || menu !== null || phase !== "playing";
  const heading = ref ? (ref.mediaType === "series" ? `${ref.title} · ${epLabel(ref.season, ref.episode)}${session?.episodeTitle ? ` · ${session.episodeTitle}` : ""}` : ref.title) : "";
  const VolIcon = muted || volume === 0 ? VolumeX : volume < 50 ? Volume1 : Volume2;
  const lowerQuality = useMemo(() => streams.findIndex((s) => !s.multi && s.height > 0 && s.height <= 720 && s !== current), [streams, current]);

  const seekFromEvent = (clientX: number) => {
    const el = seekRef.current;
    if (!el || duration <= 0) return;
    const r = el.getBoundingClientRect();
    const f = clamp((clientX - r.left) / r.width, 0, 1);
    mpv.seek(f * duration, "absolute").catch(() => {});
  };

  if (mini) {
    return (
      <div className="group fixed inset-0 select-none" onMouseMove={poke} onDoubleClick={toggleMini}>
        <div className={`absolute inset-0 flex items-center justify-center gap-3 bg-black/40 transition-opacity ${showControls ? "opacity-100" : "opacity-0"}`}>
          <button className="rounded-full bg-black/60 p-3 hover:bg-white/20" onClick={() => seekBy(-10)} title="Back 10 seconds">
            <RotateCcw size={22} />
          </button>
          <button className="rounded-full bg-white p-4 text-black" onClick={togglePause} title={paused ? "Play" : "Pause"}>
            {paused ? <Play size={26} fill="currentColor" /> : <Pause size={26} fill="currentColor" />}
          </button>
          <button className="rounded-full bg-black/60 p-3 hover:bg-white/20" onClick={() => seekBy(10)} title="Forward 10 seconds">
            <RotateCw size={22} />
          </button>
          <button className="absolute right-2 top-2 rounded-full bg-black/60 p-2 hover:bg-white/20" onClick={toggleMini} title="Back to full player (Shift+M)">
            <Maximize size={18} />
          </button>
          <div className="absolute inset-x-0 bottom-0 h-1 bg-white/20">
            <div className="h-full bg-brand" style={{ width: `${pct}%` }} />
          </div>
        </div>
      </div>
    );
  }

  return (
    <div
      className={`fixed inset-0 select-none ${showControls ? "" : "cursor-none"}`}
      onMouseMove={poke}
      onWheel={(e) => setVol(live.current.volume + (e.deltaY < 0 ? 5 : -5))}
      onClick={() => setMenu(null)}
    >
      {/* click / double-click surface */}
      <div
        className="absolute inset-0"
        onClick={(e) => {
          e.stopPropagation();
          if (menu) {
            setMenu(null);
            return;
          }
          window.clearTimeout(clickTimer.current);
          clickTimer.current = window.setTimeout(() => phase === "playing" && togglePause(), 220);
        }}
        onDoubleClick={() => {
          window.clearTimeout(clickTimer.current);
          toggleFs();
        }}
      />

      {/* loading / buffering */}
      {(phase === "resolving" || phase === "loading" || (phase === "playing" && buffering)) && (
        <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-3 bg-black/20">
          <Loader2 size={64} className="animate-spin text-white/90" />
          <div className="rounded-full bg-black/60 px-4 py-1.5 text-sm text-white/85">
            {phase === "resolving" ? "Finding the best quality…" : phase === "loading" ? "Starting video…" : `Buffering${bufPct != null ? ` ${bufPct}%` : "…"}${cacheSpeed > 0 ? ` · ${fmtBytes(cacheSpeed)}/s` : ""}`}
          </div>
        </div>
      )}
      {(phase === "resolving" || phase === "loading") && <div className="pointer-events-none absolute inset-0 -z-10 bg-black" />}

      {/* resume prompt */}
      {phase === "resume" && (
        <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/80">
          <div className="fade-in w-[420px] rounded-2xl border border-white/10 bg-panel p-7 text-center shadow-2xl">
            <h2 className="text-2xl font-bold">Welcome back</h2>
            <p className="mt-2 text-white/65">You stopped at {fmtTime(resumeAt)}.</p>
            <div className="mt-6 flex flex-col gap-3">
              <button autoFocus onClick={() => chooseResume(true)} className="h-12 rounded-xl bg-white font-bold text-black hover:bg-white/85">
                ▶ Continue from {fmtTime(Math.max(0, resumeAt - 5))}
              </button>
              <button onClick={() => chooseResume(false)} className="h-12 rounded-xl bg-white/10 font-semibold hover:bg-white/20">
                Start over
              </button>
            </div>
          </div>
        </div>
      )}

      {/* error */}
      {phase === "error" && (
        <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/85">
          <div className="fade-in max-w-md rounded-2xl border border-white/10 bg-panel p-7 text-center">
            <div className="mb-3 text-5xl">😕</div>
            <p className="text-lg text-white/85">{errorMsg}</p>
            <div className="mt-6 flex justify-center gap-3">
              <button onClick={leave} className="h-11 rounded-xl bg-white/10 px-5 font-semibold hover:bg-white/20">
                Back
              </button>
              {!session?.localPath && (
                <button
                  onClick={() => {
                    attempt.current.triedAlt = false;
                    attempt.current.index = streamsRef.current.length;
                    handleFailure();
                  }}
                  className="h-11 rounded-xl bg-white px-5 font-bold text-black hover:bg-white/85"
                >
                  Try another source
                </button>
              )}
              <button onClick={() => session && startSession({ ...session })} className="h-11 rounded-xl bg-white/10 px-5 font-semibold hover:bg-white/20">
                Try again
              </button>
            </div>
          </div>
        </div>
      )}

      {/* OSD */}
      {osd && (
        <div className="pointer-events-none absolute left-1/2 top-[18%] z-40 -translate-x-1/2 rounded-xl bg-black/70 px-5 py-2.5 text-xl font-semibold backdrop-blur">{osd}</div>
      )}

      {/* top bar */}
      <div className={`absolute inset-x-0 top-0 z-30 flex items-center gap-3 bg-gradient-to-b from-black/80 to-transparent px-5 pb-10 pt-4 transition-opacity duration-300 ${showControls ? "opacity-100" : "pointer-events-none opacity-0"}`} onClick={(e) => e.stopPropagation()}>
        <button onClick={leave} title="Back" aria-label="Back" className="rounded-full p-2 hover:bg-white/15">
          <ArrowLeft size={28} />
        </button>
        <div className="min-w-0 flex-1 truncate text-lg font-semibold">{heading}</div>
        <button onClick={() => setShowKeys(true)} title="Keyboard shortcuts (?)" className="rounded-full p-2 hover:bg-white/15">
          <Keyboard size={22} />
        </button>
      </div>

      {/* stall offer */}
      {stallOffer && lowerQuality >= 0 && phase === "playing" && (
        <div className="fade-in absolute right-6 top-20 z-40 w-80 rounded-2xl border border-white/10 bg-black/90 p-4 shadow-2xl" onClick={(e) => e.stopPropagation()}>
          <p className="font-semibold">The video keeps stopping to load.</p>
          <p className="mt-1 text-sm text-white/60">A lower quality usually plays smoothly on a slow connection.</p>
          <div className="mt-3 flex gap-2">
            <button className="h-9 flex-1 rounded-lg bg-white font-bold text-black" onClick={() => switchQuality(lowerQuality)}>
              Switch to {streams[lowerQuality].label}
            </button>
            <button className="h-9 rounded-lg bg-white/10 px-3" onClick={() => setStallOffer(false)}>
              No thanks
            </button>
          </div>
        </div>
      )}

      {/* sleep finished */}
      {sleepDone && (
        <div className="fade-in absolute left-1/2 top-1/2 z-40 -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-white/10 bg-black/90 p-6 text-center" onClick={(e) => e.stopPropagation()}>
          <MoonStar className="mx-auto mb-2" size={36} />
          <p className="text-lg font-semibold">Sleep timer finished</p>
          <button className="mt-4 h-10 rounded-lg bg-white px-5 font-bold text-black" onClick={() => { setSleepDone(false); mpv.set("pause", false); }}>
            Keep watching
          </button>
        </div>
      )}

      {/* up next */}
      {upNext !== null && next && (
        <div className="fade-in absolute bottom-32 right-6 z-40 w-80 rounded-2xl border border-white/10 bg-black/90 p-4 shadow-2xl" onClick={(e) => e.stopPropagation()}>
          <div className="text-xs font-bold uppercase tracking-widest text-white/50">Up next in {upNext}</div>
          <div className="mt-1 font-semibold">
            {epLabel(next.season, next.episode)} · {next.title || `Episode ${next.episode}`}
          </div>
          <div className="mt-3 flex gap-2">
            <button className="h-9 flex-1 rounded-lg bg-white font-bold text-black" onClick={() => { setUpNext(null); goTo(next, 0); }}>
              ▶ Play now
            </button>
            <button className="h-9 rounded-lg bg-white/10 px-4" onClick={() => { setUpNext(null); setUpNextDismissed(true); }}>
              Cancel
            </button>
          </div>
        </div>
      )}

      {/* info */}
      {showInfo && (
        <div className="fade-in absolute left-6 top-20 z-40 w-80 rounded-2xl border border-white/10 bg-black/85 p-4 text-sm" onClick={(e) => e.stopPropagation()}>
          <div className="mb-2 flex items-center justify-between font-bold">
            Video information
            <button onClick={() => setShowInfo(false)} className="rounded-full p-1 hover:bg-white/10" aria-label="Close">
              <X size={14} />
            </button>
          </div>
          <dl className="grid grid-cols-[110px_1fr] gap-y-1 text-white/75">
            <dt className="text-white/45">Quality</dt>
            <dd>{current?.label ?? "—"}</dd>
            <dt className="text-white/45">Picture</dt>
            <dd>{videoParams?.w ? `${videoParams.w} × ${videoParams.h}` : "—"}</dd>
            <dt className="text-white/45">Codec</dt>
            <dd>{current?.codec ?? tracks.find((t) => t.type === "video" && t.selected)?.codec ?? "—"}</dd>
            <dt className="text-white/45">File size</dt>
            <dd>{fmtBytes(current?.size)}</dd>
            <dt className="text-white/45">Source</dt>
            <dd>{current?.source ?? "—"}</dd>
            <dt className="text-white/45">Buffered</dt>
            <dd>{cacheTime > pos ? `${Math.round(cacheTime - pos)} s ahead` : "—"}</dd>
          </dl>
        </div>
      )}

      {/* shortcuts */}
      {showKeys && (
        <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/80" onClick={(e) => { e.stopPropagation(); setShowKeys(false); }}>
          <div className="fade-in max-h-[85vh] w-[640px] overflow-y-auto rounded-2xl border border-white/10 bg-panel p-6" onClick={(e) => e.stopPropagation()}>
            <div className="mb-4 flex items-center justify-between">
              <h2 className="text-xl font-bold">Keyboard shortcuts</h2>
              <button onClick={() => setShowKeys(false)} className="rounded-full p-2 hover:bg-white/10" aria-label="Close">
                <X size={18} />
              </button>
            </div>
            <div className="grid grid-cols-2 gap-x-6 gap-y-2 text-sm">
              {PLAYER_SHORTCUTS.map(([k, v]) => (
                <div key={k} className="flex items-center justify-between gap-3 border-b border-white/5 py-1.5">
                  <span className="text-white/70">{v}</span>
                  <kbd className="whitespace-nowrap rounded-md bg-white/10 px-2 py-0.5 font-mono text-xs">{k}</kbd>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* episode drawer */}
      {drawer && session && (
        <div className="fade-in absolute bottom-0 right-0 top-0 z-40 w-[360px] overflow-y-auto border-l border-white/10 bg-black/90 p-4 backdrop-blur" onClick={(e) => e.stopPropagation()}>
          <div className="mb-3 flex items-center justify-between">
            <h3 className="text-lg font-bold">Episodes</h3>
            <button onClick={() => setDrawer(false)} className="rounded-full p-2 hover:bg-white/10" aria-label="Close">
              <X size={18} />
            </button>
          </div>
          {[...session.seasons].sort((a, b) => a.number - b.number).map((s) => (
            <div key={s.number} className="mb-4">
              <div className="mb-1 text-sm font-bold uppercase tracking-wider text-white/45">Season {s.number}</div>
              {s.episodes.map((e) => {
                const isCur = s.number === ref?.season && e.number === ref?.episode;
                return (
                  <button key={e.number} onClick={() => { setDrawer(false); goTo({ season: s.number, episode: e.number, title: e.title }, null); }} className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left hover:bg-white/10 ${isCur ? "bg-white/15 font-semibold" : "text-white/80"}`}>
                    <span className="w-6 text-right text-white/45">{e.number}</span>
                    <span className="flex-1 truncate">{e.title || `Episode ${e.number}`}</span>
                    {isCur && <Play size={14} fill="currentColor" />}
                  </button>
                );
              })}
            </div>
          ))}
        </div>
      )}

      {/* menus */}
      {menu === "speed" && (
        <MenuPanel title="Playback speed" onClose={() => setMenu(null)}>
          {SPEEDS.map((s) => (
            <MenuItem key={s} active={Math.abs(s - speed) < 0.001} onClick={() => setSpd(s)}>
              {s === 1 ? "Normal (1x)" : `${s}x`}
            </MenuItem>
          ))}
        </MenuPanel>
      )}
      {menu === "subs" && (
        <MenuPanel title="Subtitles" onClose={() => setMenu(null)}>
          <MenuItem active={!subTracks.some((t) => t.selected)} onClick={() => { mpv.set("sid", "no"); flash("Subtitles off"); }}>
            Off
          </MenuItem>
          {subTracks.map((t) => (
            <MenuItem key={t.id} active={t.selected} onClick={() => { mpv.set("sid", t.id); flash(`Subtitles: ${t.title || t.lang || t.id}`); }}>
              {t.title || t.lang || `Track ${t.id}`}
            </MenuItem>
          ))}
          {subs.filter((s) => s.url !== activeOnlineSub).length > 0 && <div className="mx-3 my-2 border-t border-white/10 pt-2 text-xs uppercase tracking-wider text-white/40">More languages</div>}
          {subs
            .filter((s) => s.url !== activeOnlineSub)
            .map((s) => (
              <MenuItem key={s.url} onClick={() => selectOnlineSub(s)}>
                {s.name}
              </MenuItem>
            ))}
          <div className="mx-3 my-2 border-t border-white/10" />
          <MenuItem onClick={loadSubFile}>Load from file…</MenuItem>
          <div className="flex items-center justify-between px-3 py-2 text-sm text-white/60">
            <span>Timing {subDelay.toFixed(1)} s</span>
            <span className="flex gap-1">
              <button className="rounded bg-white/10 px-2 py-0.5 hover:bg-white/20" onClick={() => mpv.set("sub-delay", Math.round((subDelay - 0.1) * 10) / 10)}>−</button>
              <button className="rounded bg-white/10 px-2 py-0.5 hover:bg-white/20" onClick={() => mpv.set("sub-delay", Math.round((subDelay + 0.1) * 10) / 10)}>+</button>
            </span>
          </div>
        </MenuPanel>
      )}
      {menu === "audio" && (
        <MenuPanel title="Audio" onClose={() => setMenu(null)}>
          {audioTracks.map((t) => (
            <MenuItem key={t.id} active={t.selected} onClick={() => { mpv.set("aid", t.id); flash(`Audio: ${t.title || t.lang || t.id}`); }}>
              {t.title || t.lang || `Track ${t.id}`}
            </MenuItem>
          ))}
          {audioTracks.length === 0 && <div className="px-3 py-2 text-white/50">No audio tracks</div>}
          <div className="mx-3 my-2 border-t border-white/10" />
          <MenuItem active={night} onClick={toggleNight} sub="evens out volume">
            Night mode
          </MenuItem>
        </MenuPanel>
      )}
      {menu === "quality" && (
        <MenuPanel title="Quality" onClose={() => setMenu(null)}>
          {streams.map((s, i) => (
            <MenuItem key={s.url + i} active={s === current} onClick={() => switchQuality(i)} sub={s.size ? fmtBytes(s.size) : s.source}>
              {s.multi ? "Auto (best)" : s.label}
            </MenuItem>
          ))}
        </MenuPanel>
      )}
      {menu === "sleep" && (
        <MenuPanel title="Sleep timer" onClose={() => setMenu(null)}>
          <MenuItem active={!sleep} onClick={() => { setSleep(null); flash("Sleep timer off"); setMenu(null); }}>
            Off
          </MenuItem>
          {[15, 30, 60, 90].map((m) => (
            <MenuItem key={m} active={sleep?.kind === "time" && sleep.minutes === m} onClick={() => { setSleep({ kind: "time", at: Date.now() + m * 60000, minutes: m }); flash(`Sleep in ${m} minutes`); setMenu(null); }}>
              {m} minutes
            </MenuItem>
          ))}
          <MenuItem active={sleep?.kind === "episode"} onClick={() => { setSleep({ kind: "episode" }); flash("Stop at the end of this"); setMenu(null); }}>
            End of this {isSeries ? "episode" : "movie"}
          </MenuItem>
        </MenuPanel>
      )}

      {/* bottom controls */}
      <div className={`absolute inset-x-0 bottom-0 z-30 bg-gradient-to-t from-black/90 via-black/60 to-transparent px-6 pb-4 pt-16 transition-opacity duration-300 ${showControls ? "opacity-100" : "pointer-events-none opacity-0"}`} onClick={(e) => e.stopPropagation()}>
        {/* seek bar */}
        <div
          ref={seekRef}
          className="group/seek relative mb-3 flex h-5 cursor-pointer items-center"
          onMouseMove={(e) => {
            const r = e.currentTarget.getBoundingClientRect();
            setHoverX(clamp(e.clientX - r.left, 0, r.width));
          }}
          onMouseLeave={() => setHoverX(null)}
          onMouseDown={(e) => {
            seekFromEvent(e.clientX);
            const move = (ev: MouseEvent) => seekFromEvent(ev.clientX);
            const up = () => {
              window.removeEventListener("mousemove", move);
              window.removeEventListener("mouseup", up);
            };
            window.addEventListener("mousemove", move);
            window.addEventListener("mouseup", up);
          }}
        >
          <div className="relative h-1.5 w-full overflow-hidden rounded-full bg-white/20 transition-all group-hover/seek:h-2.5">
            <div className="absolute inset-y-0 left-0 bg-white/35" style={{ width: `${bufferedPct}%` }} />
            <div className="absolute inset-y-0 left-0 bg-brand" style={{ width: `${pct}%` }} />
            {duration > 0 && chapters.map((c, i) => (i > 0 ? <div key={i} className="absolute inset-y-0 w-0.5 bg-black/70" style={{ left: `${(c.time / duration) * 100}%` }} /> : null))}
          </div>
          <div className="pointer-events-none absolute h-4 w-4 -translate-x-1/2 rounded-full bg-brand opacity-0 shadow group-hover/seek:opacity-100" style={{ left: `${pct}%` }} />
          {hoverX !== null && duration > 0 && seekRef.current && (
            <div className="pointer-events-none absolute -top-9 -translate-x-1/2 rounded-md bg-black/90 px-2 py-1 text-sm font-semibold" style={{ left: hoverX }}>
              {fmtTime((hoverX / seekRef.current.getBoundingClientRect().width) * duration)}
            </div>
          )}
        </div>

        <div className="flex items-center gap-1">
          <button className="rounded-full p-2.5 hover:bg-white/15" onClick={togglePause} title={paused ? "Play (Space)" : "Pause (Space)"} aria-label={paused ? "Play" : "Pause"}>
            {paused ? <Play size={30} fill="currentColor" /> : <Pause size={30} fill="currentColor" />}
          </button>
          <button className="rounded-full p-2.5 hover:bg-white/15" onClick={() => seekBy(-10)} title="Back 10 seconds (←)" aria-label="Back 10 seconds">
            <RotateCcw size={24} />
          </button>
          <button className="rounded-full p-2.5 hover:bg-white/15" onClick={() => seekBy(10)} title="Forward 10 seconds (→)" aria-label="Forward 10 seconds">
            <RotateCw size={24} />
          </button>
          {isSeries && (
            <>
              <button className="rounded-full p-2.5 hover:bg-white/15 disabled:opacity-30" disabled={!prev} onClick={() => goTo(prev, 0)} title="Previous episode (P)" aria-label="Previous episode">
                <SkipBack size={22} />
              </button>
              <button className="rounded-full p-2.5 hover:bg-white/15 disabled:opacity-30" disabled={!next} onClick={() => goTo(next, 0)} title="Next episode (N)" aria-label="Next episode">
                <SkipForward size={22} />
              </button>
            </>
          )}
          <div className="group/vol ml-1 flex items-center">
            <button className="rounded-full p-2.5 hover:bg-white/15" onClick={() => mpv.set("mute", !muted)} title="Mute (M)" aria-label="Mute">
              <VolIcon size={24} />
            </button>
            <input
              type="range"
              min={0}
              max={150}
              value={muted ? 0 : volume}
              onChange={(e) => setVol(Number(e.target.value))}
              aria-label="Volume"
              className="mb-range w-0 opacity-0 transition-all group-hover/vol:w-28 group-hover/vol:opacity-100"
              style={{ ["--pct" as string]: `${((muted ? 0 : volume) / 150) * 100}%` }}
            />
          </div>
          <button className="ml-2 rounded px-2 py-1 font-mono text-[15px] text-white/85 hover:bg-white/10" onClick={() => setRemaining((v) => !v)} title="Show time left / elapsed">
            {remaining ? `−${fmtTime(duration - pos)}` : fmtTime(pos)} / {fmtTime(duration)}
          </button>

          <div className="flex-1" />

          {sleep && <span className="mr-1 flex items-center gap-1 rounded-full bg-white/10 px-2 py-1 text-xs text-white/75"><Timer size={14} />{sleep.kind === "time" ? `${Math.max(0, Math.ceil((sleep.at - Date.now()) / 60000))} min` : "End"}</span>}
          <button className={`rounded-full p-2.5 hover:bg-white/15 ${speed !== 1 ? "text-brand2" : ""}`} onClick={(e) => { e.stopPropagation(); setMenu(menu === "speed" ? null : "speed"); }} title="Playback speed ([ and ])" aria-label="Playback speed">
            <span className="flex items-center gap-1"><Gauge size={22} />{speed !== 1 && <span className="text-sm font-bold">{speed}x</span>}</span>
          </button>
          <button className="rounded-full p-2.5 hover:bg-white/15" onClick={(e) => { e.stopPropagation(); setMenu(menu === "subs" ? null : "subs"); }} title="Subtitles (C)" aria-label="Subtitles">
            <Captions size={24} />
          </button>
          <button className="rounded-full p-2.5 hover:bg-white/15" onClick={(e) => { e.stopPropagation(); setMenu(menu === "audio" ? null : "audio"); }} title="Audio and night mode (A)" aria-label="Audio">
            <AudioLines size={24} />
          </button>
          {!session?.localPath && (
            <button className="rounded-full p-2.5 hover:bg-white/15" onClick={(e) => { e.stopPropagation(); setMenu(menu === "quality" ? null : "quality"); }} title="Quality" aria-label="Quality">
              <span className="flex items-center gap-1"><Settings2 size={22} /><span className="text-xs font-bold">{current?.multi ? "AUTO" : current?.label}</span></span>
            </button>
          )}
          <button className={`rounded-full p-2.5 hover:bg-white/15 ${sleep ? "text-brand2" : ""}`} onClick={(e) => { e.stopPropagation(); setMenu(menu === "sleep" ? null : "sleep"); }} title="Sleep timer" aria-label="Sleep timer">
            <Moon size={22} />
          </button>
          {isSeries && (
            <button className={`rounded-full p-2.5 hover:bg-white/15 ${drawer ? "text-brand2" : ""}`} onClick={() => setDrawer((v) => !v)} title="Episodes (E)" aria-label="Episodes">
              <ListVideo size={24} />
            </button>
          )}
          <button className="rounded-full p-2.5 hover:bg-white/15" onClick={() => setShowInfo((v) => !v)} title="Video information (I)" aria-label="Video information">
            <Info size={22} />
          </button>
          <button className={`rounded-full p-2.5 hover:bg-white/15 ${onTop ? "text-brand2" : ""}`} onClick={toggleTop} title="Keep on top (T)" aria-label="Keep on top">
            <Pin size={22} />
          </button>
          <button className="rounded-full p-2.5 hover:bg-white/15" onClick={toggleMini} title="Mini player (Shift+M)" aria-label="Mini player">
            <PictureInPicture2 size={22} />
          </button>
          <button className="rounded-full p-2.5 hover:bg-white/15" onClick={toggleFs} title="Full screen (F)" aria-label="Full screen">
            {fullscreen ? <Minimize size={24} /> : <Maximize size={24} />}
          </button>
          <ChevronRight className="hidden" />
        </div>
      </div>
    </div>
  );
}
