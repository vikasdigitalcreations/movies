import { useEffect, useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import { api, type Addon } from "../lib/api";
import { Button, ConfirmDialog, Spinner, Toggle } from "./ui";
import { useApp } from "../store/app";

/**
 * Settings -> Extra sources.
 *
 * MovieBox and 4KHDHub are built in and go dark without warning; an addon is a source
 * the user adds themselves, maintained by whoever wrote it. This panel is deliberately
 * plain: paste a link, see whether it works, turn it off again.
 */
export function AddonsSettings() {
  const [list, setList] = useState<Addon[] | null>(null);
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [remove, setRemove] = useState<Addon | null>(null);
  const toast = useApp((s) => s.toast);

  const load = () => api.addonsList().then(setList).catch(() => setList([]));
  useEffect(() => {
    load();
  }, []);

  const add = async () => {
    if (!url.trim() || busy) return;
    setBusy(true);
    try {
      const a = await api.addonsAdd(url.trim());
      setUrl("");
      await load();
      toast(
        a.providesStream ? `Added ${a.name}. It will be used when the built-in sources have nothing.` : `Added ${a.name}.`,
        "success",
      );
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const toggle = async (a: Addon, enabled: boolean) => {
    setList((cur) => cur?.map((x) => (x.manifestUrl === a.manifestUrl ? { ...x, enabled } : x)) ?? cur);
    await api.addonsToggle(a.manifestUrl, enabled).catch(() => load());
  };

  const doRemove = async () => {
    if (!remove) return;
    try {
      await api.addonsRemove(remove.manifestUrl);
      await load();
      toast(`Removed ${remove.name}.`, "success");
    } catch (e) {
      toast(String(e), "error");
    }
    setRemove(null);
  };

  return (
    <div className="px-4 py-4">
      <p className="mb-4 text-sm text-white/55">
        Addons are extra places to find a title, added by you. MovieBox is tried first, then 4KHDHub, then these. Paste an
        addon's link below — it stays on this PC and nothing about you is sent to it.
      </p>

      <div className="mb-5 flex gap-2">
        <input
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
          placeholder="https://example.com/manifest.json"
          spellCheck={false}
          className="min-w-0 flex-1 rounded-xl border border-white/10 bg-black/30 px-3 py-2 text-sm outline-none placeholder:text-white/30 focus:border-white/25"
        />
        <Button variant="primary" size="sm" onClick={add} disabled={busy || !url.trim()}>
          {busy ? <Spinner size={16} /> : <Plus size={16} />}
          Add
        </Button>
      </div>

      {list === null ? (
        <Spinner size={20} />
      ) : (
        <div className="space-y-2">
          {list.map((a) => (
            <div key={a.manifestUrl} className="flex items-center gap-4 rounded-xl border border-white/6 bg-black/20 px-3 py-3">
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-semibold">{a.name}</span>
                  {a.version && <span className="text-xs text-white/35">v{a.version}</span>}
                  {a.providesStream && <span className="rounded-md bg-brand/20 px-1.5 py-0.5 text-xs text-brand">streams</span>}
                  {a.core && <span className="rounded-md bg-white/10 px-1.5 py-0.5 text-xs text-white/50">needed for matching</span>}
                </div>
                {a.description && <div className="truncate text-sm text-white/45">{a.description}</div>}
              </div>
              <Toggle checked={a.enabled} onChange={(v) => toggle(a, v)} label={`Use ${a.name}`} />
              {!a.core && (
                <button
                  onClick={() => setRemove(a)}
                  title={`Remove ${a.name}`}
                  className="rounded-lg p-2 text-white/40 transition hover:bg-white/10 hover:text-white"
                >
                  <Trash2 size={16} />
                </button>
              )}
            </div>
          ))}
          {list.filter((a) => a.providesStream).length === 0 && (
            <p className="pt-1 text-sm text-white/40">
              No streaming addon yet. Cinemeta only matches titles to addons — it doesn't play anything on its own.
            </p>
          )}
        </div>
      )}

      <ConfirmDialog
        open={remove !== null}
        title={`Remove ${remove?.name ?? ""}?`}
        text="MovieBox will stop looking there for titles. You can add it again at any time."
        confirmLabel="Remove"
        onConfirm={doRemove}
        onClose={() => setRemove(null)}
      />
    </div>
  );
}
