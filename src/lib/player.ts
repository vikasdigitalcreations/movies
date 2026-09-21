import {
  init,
  command,
  setProperty,
  getProperty,
  observeProperties,
  listenEvents,
  destroy,
  type MpvObservableProperty,
} from "tauri-plugin-libmpv-api";

export const OBSERVED = [
  ["pause", "flag"],
  ["time-pos", "double", "none"],
  ["duration", "double", "none"],
  ["volume", "double"],
  ["mute", "flag"],
  ["speed", "double"],
  ["paused-for-cache", "flag", "none"],
  ["cache-buffering-state", "int64", "none"],
  ["demuxer-cache-time", "double", "none"],
  ["cache-speed", "int64", "none"],
  ["eof-reached", "flag", "none"],
  ["track-list", "node", "none"],
  ["sid", "node", "none"],
  ["aid", "node", "none"],
  ["sub-delay", "double"],
  ["chapter-list", "node", "none"],
  ["video-params", "node", "none"],
  ["idle-active", "flag"],
] as const satisfies MpvObservableProperty[];

export interface Track {
  id: number;
  type: "video" | "audio" | "sub";
  title?: string;
  lang?: string;
  selected?: boolean;
  external?: boolean;
  codec?: string;
  "demux-w"?: number;
  "demux-h"?: number;
}

let initialized = false;

export async function playerInit(opts: { volume: number; subSize: number; subBackground: boolean }) {
  if (initialized) return;
  try {
    await destroy();
  } catch {
    /* no previous instance */
  }
  await init({
    initialOptions: {
      vo: "gpu-next",
      "gpu-context": "d3d11",
      hwdec: "auto-safe",
      "keep-open": "yes",
      "force-window": "yes",
      idle: "yes",
      "input-default-bindings": "no",
      "input-vo-keyboard": "no",
      osc: "no",
      "osd-level": "0",
      cache: "yes",
      "demuxer-max-bytes": "256MiB",
      "demuxer-max-back-bytes": "64MiB",
      "demuxer-readahead-secs": "60",
      "cache-pause-initial": "yes",
      "cache-pause-wait": "3",
      "hr-seek": "yes",
      "network-timeout": "30",
      "audio-pitch-correction": "yes",
      af: "scaletempo2",
      "volume-max": "150",
      volume: String(Math.round(opts.volume)),
      "sub-font-size": String(opts.subSize),
      "sub-border-size": "2.5",
      "sub-back-color": opts.subBackground ? "#99000000" : "#00000000",
      "sub-border-style": opts.subBackground ? "background-box" : "outline-and-shadow",
      "sub-auto": "fuzzy",
      slang: "en,eng,English",
      "screenshot-format": "png",
      "ytdl": "no",
      "load-scripts": "no",
    },
    observedProperties: OBSERVED,
  });
  initialized = true;
}

export async function playerDestroy() {
  if (!initialized) return;
  initialized = false;
  try {
    await destroy();
  } catch {
    /* ignore */
  }
}

export function onProps(cb: Parameters<typeof observeProperties<typeof OBSERVED>>[1]) {
  return observeProperties(OBSERVED, cb);
}

export function onEvents(cb: Parameters<typeof listenEvents>[0]) {
  return listenEvents(cb);
}

function headerOption(headers: [string, string][]) {
  const ua = headers.find(([k]) => k.toLowerCase() === "user-agent")?.[1];
  const referer = headers.find(([k]) => k.toLowerCase() === "referer")?.[1];
  const rest = headers
    .filter(([k]) => !["user-agent", "referer"].includes(k.toLowerCase()))
    .map(([k, v]) => `${k}: ${v.replace(/,/g, "\\,")}`);
  return { ua, referer, fields: rest.join(",") };
}

/**
 * Waits until mpv has finished any seek it is in the middle of.
 *
 * mpv deadlocks if the current file is torn down while it is still seeking, and every
 * resume does exactly that on a DASH stream: `start=N` is a seek. The core thread ends up
 * waiting forever on the demuxer, and from then on each `loadfile` is accepted but never
 * begins opening, so the player is dead until the app restarts. Reproduced with the
 * bundled libmpv by loading a stream at start=4217 and sending `stop` about 1.5 s later;
 * waiting for `seeking` to clear first made the same sequence safe every time. So anything
 * that ends the current file -- leaving the player, switching quality or episode -- settles
 * first. It is bounded, so a seek that never ends cannot hold the app up.
 */
async function settle(maxMs = 8000) {
  const t0 = Date.now();
  while (Date.now() - t0 < maxMs) {
    let seeking = false;
    try {
      seeking = (await getProperty("seeking", "flag")) === true;
    } catch {
      return;
    }
    if (!seeking) return;
    await new Promise((r) => setTimeout(r, 50));
  }
}

export async function load(url: string, headers: [string, string][], start: number) {
  await settle();
  const { ua, referer, fields } = headerOption(headers);
  // The CDN rejects browser-like user agents that lack browser headers (HTTP 428), so fall back to libmpv's own.
  await setProperty("user-agent", ua ?? "libmpv");
  await setProperty("referrer", referer ?? "");
  await setProperty("http-header-fields", fields);
  await setProperty("pause", false);
  const opts = start > 1 ? `start=${Math.floor(start)}` : "start=0";
  await command("loadfile", [url, "replace", "-1", opts]);
}

export const mpv = {
  command,
  set: setProperty,
  get: getProperty,
  stop: async () => {
    // Pause first: the seek being waited out is about to finish and start playing, and
    // nothing should be heard from a player the person has already left.
    await setProperty("pause", true).catch(() => {});
    await settle();
    await command("stop", []);
  },
  seek: (sec: number, mode: "relative" | "absolute" | "absolute-percent" = "relative") =>
    command("seek", [String(sec), mode]),
  frameStep: () => command("frame-step", []),
  frameBackStep: () => command("frame-back-step", []),
  addSub: (path: string, title: string) => command("sub-add", [path, "select", title]),
  screenshot: (path: string) => command("screenshot-to-file", [path, "subtitles"]),
};
