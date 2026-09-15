import { useEffect, useState } from "react";
import { init, command, observeProperties, setProperty } from "tauri-plugin-libmpv-api";

const PROPS = [["pause", "flag"], ["time-pos", "double", "none"], ["duration", "double", "none"]] as const;

export default function App() {
  const [status, setStatus] = useState("starting");
  const [pos, setPos] = useState(0);
  const [paused, setPaused] = useState(false);
  useEffect(() => {
    (async () => {
      try {
        await init({ initialOptions: { vo: "gpu-next", hwdec: "auto-safe", "keep-open": "yes", "force-window": "yes" }, observedProperties: PROPS });
        await observeProperties(PROPS, ({ name, data }) => {
          if (name === "time-pos") setPos((data as number) ?? 0);
          if (name === "pause") setPaused(data as boolean);
        });
        await command("loadfile", ["https://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4"]);
        setStatus("playing");
        (window as any).__spike = { ok: true };
      } catch (e) { setStatus("error: " + String(e)); }
    })();
  }, []);
  return (
    <div style={{ position: "fixed", bottom: 20, left: 20, background: "rgba(0,0,0,.7)", color: "#fff", padding: 12, borderRadius: 8 }}>
      <span id="spike-status">{status} t={pos.toFixed(1)}</span>{" "}
      <button id="spike-toggle" onClick={() => setProperty("pause", !paused)}>{paused ? "Play" : "Pause"}</button>
    </div>
  );
}
