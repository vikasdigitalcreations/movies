import { useEffect } from "react";
import { HashRouter, Route, Routes, useLocation, useNavigate } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Sidebar } from "./components/Sidebar";
import { ContextMenu, OfflineBanner, Toasts, WelcomeTour } from "./components/Overlays";
import { HomePage } from "./pages/Home";
import { SearchPage } from "./pages/Search";
import { DetailsPage } from "./pages/Details";
import { PlayerPage } from "./pages/Player";
import { DownloadsPage } from "./pages/Downloads";
import { SettingsPage } from "./pages/Settings";
import { HelpPage } from "./pages/Help";
import { ContinuePage, MyListPage } from "./pages/Library";
import { useApp } from "./store/app";
import { isTyping, moveFocus } from "./lib/hooks";
import type { DownloadTask } from "./lib/api";

function Shell() {
  const location = useLocation();
  const navigate = useNavigate();
  const settings = useApp((s) => s.settings);
  const saveSettings = useApp((s) => s.saveSettings);
  const inPlayer = location.pathname === "/play";

  // startup data + download events
  useEffect(() => {
    const st = useApp.getState();
    st.loadSettings().then(() => {
      if (!useApp.getState().settings?.tourDone) useApp.getState().setTourOpen(true);
    });
    st.refreshDownloads();
    st.refreshHistory();
    st.refreshFavorites();
    const un1 = listen<DownloadTask>("download://progress", (e) => useApp.getState().upsertDownload(e.payload));
    const un2 = listen<string>("download://removed", (e) => useApp.getState().removeDownload(e.payload));
    return () => {
      un1.then((u) => u());
      un2.then((u) => u());
    };
  }, []);

  // UI zoom
  useEffect(() => {
    if (settings) getCurrentWebview().setZoom(settings.uiZoom).catch(() => {});
  }, [settings?.uiZoom]);

  // global keys (the player handles its own)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (inPlayer) return;
      const z = useApp.getState().settings?.uiZoom ?? 1;
      if (e.ctrlKey && (e.key === "=" || e.key === "+")) {
        e.preventDefault();
        saveSettings({ uiZoom: Math.min(2, Math.round((z + 0.1) * 10) / 10) });
        return;
      }
      if (e.ctrlKey && (e.key === "-" || e.key === "_")) {
        e.preventDefault();
        saveSettings({ uiZoom: Math.max(0.6, Math.round((z - 0.1) * 10) / 10) });
        return;
      }
      if (e.ctrlKey && e.key === "0") {
        e.preventDefault();
        saveSettings({ uiZoom: 1 });
        return;
      }
      if (e.ctrlKey && e.key.toLowerCase() === "f") {
        e.preventDefault();
        navigate("/search");
        return;
      }
      if (e.ctrlKey && e.key.toLowerCase() === "r") {
        e.preventDefault();
        return;
      }
      if (isTyping(e.target)) return;
      if (document.querySelector('[role="dialog"]')) return;
      if (e.key === "/") {
        e.preventDefault();
        navigate("/search");
      } else if (e.key === "Backspace" || (e.altKey && e.key === "ArrowLeft")) {
        e.preventDefault();
        if (location.pathname !== "/") navigate(-1);
      } else if (e.key.startsWith("Arrow") && !e.altKey && !e.ctrlKey) {
        if (moveFocus(e.key)) e.preventDefault();
      }
    };
    window.addEventListener("keydown", onKey);
    const noMenu = (e: MouseEvent) => {
      if (!(e.target as HTMLElement).closest("input,textarea")) e.preventDefault();
    };
    window.addEventListener("contextmenu", noMenu);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("contextmenu", noMenu);
    };
  }, [inPlayer, location.pathname, navigate, saveSettings]);

  if (!settings) {
    return <div className="flex h-full items-center justify-center bg-ink text-white/60">Loading MovieBox…</div>;
  }

  if (inPlayer) {
    return (
      <>
        <PlayerPage />
        <Toasts />
      </>
    );
  }

  return (
    <div className="flex h-full flex-col bg-ink">
      <OfflineBanner />
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <main className="min-w-0 flex-1">
          <Routes>
            <Route path="/" element={<HomePage tab="0" />} />
            <Route path="/movies" element={<HomePage key="movies" tab="2" title="Movies" />} />
            <Route path="/series" element={<HomePage key="series" tab="5" title="Series" />} />
            <Route path="/anime" element={<HomePage key="anime" tab="8" title="Anime" />} />
            <Route path="/search" element={<SearchPage />} />
            <Route path="/title/:id" element={<DetailsPage />} />
            <Route path="/mylist" element={<MyListPage />} />
            <Route path="/continue" element={<ContinuePage />} />
            <Route path="/downloads" element={<DownloadsPage />} />
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="/help" element={<HelpPage />} />
            <Route path="*" element={<HomePage tab="0" />} />
          </Routes>
        </main>
      </div>
      <ContextMenu />
      <WelcomeTour />
      <Toasts />
    </div>
  );
}

export default function App() {
  return (
    <HashRouter>
      <Shell />
    </HashRouter>
  );
}
