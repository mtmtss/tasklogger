import type { AppStatus } from "../types";

const STYLES: Record<AppStatus, { label: string; className: string }> = {
  not_started: { label: "未開始", className: "bg-slate-800 text-slate-400" },
  running: { label: "実行中", className: "bg-sky-500/15 text-sky-300" },
  paused: { label: "中断中", className: "bg-amber-500/15 text-amber-300" },
  completed: { label: "完了", className: "bg-emerald-500/15 text-emerald-300" },
};

export default function StatusChip({ status }: { status: AppStatus }) {
  const style = STYLES[status];
  return (
    <span
      className={`inline-block rounded-full px-2 py-0.5 text-xs font-medium ${style.className}`}
    >
      {style.label}
    </span>
  );
}
