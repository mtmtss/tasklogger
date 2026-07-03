export default function FloatWindow() {
  return (
    <div
      data-tauri-drag-region
      className="flex h-screen select-none items-center gap-3 rounded-xl border border-slate-700 bg-slate-900/95 px-4 text-slate-100"
    >
      <span className="text-sm text-slate-400">作業中タスクなし</span>
    </div>
  );
}
