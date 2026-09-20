import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { Button, Spinner, Toggle } from "./ui";
import { useApp } from "../store/app";

/**
 * Settings -> Adults. The PIN comes first on purpose: the section cannot be switched on
 * without one, and clearing the PIN switches it back off, so it can never sit unguarded.
 *
 * The PIN is a household lock, not a security measure. Anyone who can edit
 * `gui_settings.json` can remove it, and the panel says so rather than implying more.
 */
export function AdultSettings() {
  const [hasPin, setHasPin] = useState<boolean | null>(null);
  const [enabled, setEnabled] = useState(false);
  const [mode, setMode] = useState<"idle" | "set" | "clear">("idle");
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [again, setAgain] = useState("");
  const [busy, setBusy] = useState(false);
  const toast = useApp((s) => s.toast);

  const refresh = async () => {
    setHasPin(await api.pinIsSet().catch(() => false));
    setEnabled(await api.adultIsEnabled().catch(() => false));
  };
  useEffect(() => {
    refresh();
  }, []);

  const reset = () => {
    setMode("idle");
    setCurrent("");
    setNext("");
    setAgain("");
  };

  const savePin = async () => {
    if (next.length < 4) return toast("Choose a PIN of at least 4 digits.", "error");
    if (next !== again) return toast("Those two PINs don't match.", "error");
    setBusy(true);
    try {
      await api.pinSet(next, hasPin ? current : undefined);
      await refresh();
      reset();
      toast("PIN saved.", "success");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const clearPin = async () => {
    setBusy(true);
    try {
      await api.pinClear(current);
      await refresh();
      reset();
      window.dispatchEvent(new Event("moviebox:adult-changed"));
      toast("PIN removed, and the Adults section is switched off.", "success");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const toggleSection = async (v: boolean) => {
    try {
      await api.adultSetEnabled(v);
      setEnabled(v);
      window.dispatchEvent(new Event("moviebox:adult-changed"));
      toast(v ? "Adults section added to the sidebar." : "Adults section hidden.", "success");
    } catch (e) {
      toast(String(e), "error");
    }
  };

  const digits = (v: string) => v.replace(/\D/g, "").slice(0, 12);
  const field = (value: string, set: (v: string) => void, placeholder: string) => (
    <input
      type="password"
      inputMode="numeric"
      value={value}
      onChange={(e) => set(digits(e.target.value))}
      placeholder={placeholder}
      className="w-36 rounded-xl border border-white/10 bg-black/30 px-3 py-2 text-sm outline-none placeholder:text-white/30 focus:border-white/25"
    />
  );

  if (hasPin === null)
    return (
      <div className="px-4 py-4">
        <Spinner size={20} />
      </div>
    );

  return (
    <div className="px-4 py-4">
      <p className="mb-4 text-sm text-white/55">
        An 18+ section, hidden behind a PIN and kept out of Home, Search and Continue Watching. Off until you switch it on.
      </p>

      {/* PIN */}
      <div className="mb-4 rounded-xl border border-white/6 bg-black/20 px-3 py-3">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <div className="font-semibold">{hasPin ? "PIN is set" : "No PIN yet"}</div>
            <div className="text-sm text-white/45">
              {hasPin ? "Asked for once each time you open the app." : "Needed before the section can be switched on."}
            </div>
          </div>
          {mode === "idle" && (
            <div className="flex gap-2">
              <Button size="sm" onClick={() => setMode("set")}>
                {hasPin ? "Change PIN" : "Set a PIN"}
              </Button>
              {hasPin && (
                <Button size="sm" onClick={() => setMode("clear")}>
                  Remove
                </Button>
              )}
            </div>
          )}
        </div>

        {mode === "set" && (
          <div className="mt-3 flex flex-wrap items-center gap-2">
            {hasPin && field(current, setCurrent, "Current")}
            {field(next, setNext, "New PIN")}
            {field(again, setAgain, "Repeat")}
            <Button variant="primary" size="sm" onClick={savePin} disabled={busy}>
              {busy ? <Spinner size={14} /> : "Save"}
            </Button>
            <Button size="sm" onClick={reset}>
              Cancel
            </Button>
          </div>
        )}

        {mode === "clear" && (
          <div className="mt-3 flex flex-wrap items-center gap-2">
            {field(current, setCurrent, "Current PIN")}
            <Button variant="primary" size="sm" onClick={clearPin} disabled={busy || !current}>
              {busy ? <Spinner size={14} /> : "Remove PIN"}
            </Button>
            <Button size="sm" onClick={reset}>
              Cancel
            </Button>
          </div>
        )}
      </div>

      {/* the section itself */}
      <div className="flex items-center justify-between gap-6 rounded-xl border border-white/6 bg-black/20 px-3 py-3">
        <div className="min-w-0">
          <div className="font-semibold">Show the Adults section</div>
          <div className="text-sm text-white/45">
            {hasPin ? "Adds it to the sidebar, behind your PIN." : "Set a PIN first."}
          </div>
        </div>
        <Toggle checked={enabled} onChange={toggleSection} label="Show the Adults section" />
      </div>

      <p className="mt-4 text-xs text-white/35">
        Content comes from Eporner and RedGifs through their own public APIs — no account, and nothing about you is sent to
        them. The PIN keeps the section from being opened by accident; it does not encrypt anything, and anyone with this PC
        could remove it.
      </p>
    </div>
  );
}
