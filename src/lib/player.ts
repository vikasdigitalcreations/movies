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

export async function load(url: string, headers: [string, string][], start: number) {
  const { ua, referer, fields } = headerOption(headers);
  await setProperty("user-agent", ua ?? "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36");
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
  stop: () => command("stop", []),
  seek: (sec: number, mode: "relative" | "absolute" | "absolute-percent" = "relative") =>
    command("seek", [String(sec), mode]),
  frameStep: () => command("frame-step", []),
  frameBackStep: () => command("frame-back-step", []),
  addSub: (path: string, title: string) => command("sub-add", [path, "select", title]),
  screenshot: (path: string) => command("screenshot-to-file", [path, "subtitles"]),
};
