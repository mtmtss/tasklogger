import { HashRouter, NavLink, Navigate, Route, Routes } from "react-router";
import TodayPage from "./routes/TodayPage";
import ArchivePage from "./routes/ArchivePage";
import SettingsPage from "./routes/SettingsPage";

const navItems = [
  { to: "/today", label: "今日" },
  { to: "/archive", label: "アーカイブ" },
  { to: "/settings", label: "設定" },
];

export default function App() {
  return (
    <HashRouter>
      <div className="min-h-screen bg-slate-950 text-slate-100">
        <nav className="sticky top-0 z-10 flex items-center gap-1 border-b border-slate-800 bg-slate-950/90 px-4 py-2 backdrop-blur">
          <span className="mr-4 text-sm font-bold tracking-wide text-sky-400">
            TaskLogger
          </span>
          {navItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              className={({ isActive }) =>
                `rounded-md px-3 py-1.5 text-sm transition-colors ${
                  isActive
                    ? "bg-slate-800 text-sky-300"
                    : "text-slate-400 hover:bg-slate-900 hover:text-slate-200"
                }`
              }
            >
              {item.label}
            </NavLink>
          ))}
        </nav>
        <main className="mx-auto max-w-5xl px-4 py-6">
          <Routes>
            <Route path="/" element={<Navigate to="/today" replace />} />
            <Route path="/today" element={<TodayPage />} />
            <Route path="/archive" element={<ArchivePage />} />
            <Route path="/settings" element={<SettingsPage />} />
          </Routes>
        </main>
      </div>
    </HashRouter>
  );
}
