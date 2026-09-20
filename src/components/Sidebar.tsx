import { useEffect, useState } from "react";
import { NavLink } from "react-router-dom";
import { Home, Search, Film, Tv, Sparkles, Bookmark, History, Download, Settings, HelpCircle, Clapperboard, Lock } from "lucide-react";
import { api } from "../lib/api";
import { useApp } from "../store/app";

const items = [
  { to: "/", label: "Home", icon: Home, end: true },
  { to: "/search", label: "Search", icon: Search },
  { to: "/movies", label: "Movies", icon: Film },
  { to: "/series", label: "Series", icon: Tv },
  { to: "/anime", label: "Anime", icon: Sparkles },
  { to: "/mylist", label: "My List", icon: Bookmark },
  { to: "/continue", label: "Continue Watching", icon: History },
  { to: "/downloads", label: "Downloads", icon: Download, badge: true },
];

const adultItem = { to: "/adults", label: "Adults", icon: Lock };

const bottom = [
  { to: "/settings", label: "Settings", icon: Settings },
  { to: "/help", label: "Help", icon: HelpCircle },
];

export function Sidebar() {
  const active = useApp((s) => s.downloads.filter((d) => d.status === "downloading" || d.status === "queued").length);
  // The Adults entry only exists once the section is switched on and a PIN guards it,
  // so the sidebar looks untouched until then.
  const [adult, setAdult] = useState(false);
  useEffect(() => {
    const check = () => api.adultIsEnabled().then(setAdult).catch(() => setAdult(false));
    check();
    window.addEventListener("moviebox:adult-changed", check);
    return () => window.removeEventListener("moviebox:adult-changed", check);
  }, []);

  const link = (it: { to: string; label: string; icon: typeof Home; end?: boolean; badge?: boolean }) => (
    <NavLink
      key={it.to}
      to={it.to}
      end={it.end}
      title={it.label}
      className={({ isActive }) =>
        `relative flex items-center gap-3.5 rounded-xl px-3.5 py-2.5 text-[15px] font-medium transition ${
          isActive ? "bg-white/12 text-white" : "text-white/65 hover:bg-white/6 hover:text-white"
        }`
      }
    >
      {({ isActive }) => (
        <>
          {isActive && <span className="absolute left-0 top-2 bottom-2 w-1 rounded-r bg-brand" />}
          <it.icon size={22} strokeWidth={isActive ? 2.4 : 1.9} />
          <span className="truncate">{it.label}</span>
          {it.badge && active > 0 && (
            <span className="ml-auto flex h-5 min-w-5 items-center justify-center rounded-full bg-brand px-1.5 text-xs font-bold text-white">{active}</span>
          )}
        </>
      )}
    </NavLink>
  );

  return (
    <aside className="flex h-full w-[232px] shrink-0 flex-col border-r border-white/6 bg-[#0e0e13] px-3 py-5">
      <div className="mb-7 flex items-center gap-2.5 px-3">
        <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-brand">
          <Clapperboard size={20} />
        </div>
        <span className="text-xl font-black tracking-tight">MovieBox</span>
      </div>
      <nav className="flex flex-1 flex-col gap-1">
        {items.map(link)}
        {adult && link(adultItem)}
      </nav>
      <nav className="flex flex-col gap-1 border-t border-white/6 pt-3">{bottom.map(link)}</nav>
    </aside>
  );
}
