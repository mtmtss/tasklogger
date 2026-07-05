import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { stopTask } from "../lib/commands";
import { useTauriEvents } from "../lib/queries";
import { useSessionStore } from "../stores/sessionStore";
import TimerDisplay from "../components/TimerDisplay";

/** 常時最前面のミニウィジェット (spec §8.3)。 */
export default function FloatWindow() {
  useTauriEvents();
  const session = useSessionStore((s) => s.session);
  const init = useSessionStore((s) => s.init);

  useEffect(() => {
    void init();
  }, [init]);

  return (
    <div
      data-tauri-drag-region
      className="flex h-screen select-none items-center gap-3 rounded-xl border border-slate-700 bg-slate-900/95 px-4 text-slate-100"
    >
      {session ? (
        <>
          <div data-tauri-drag-region className="min-w-0 flex-1">
            <p
              data-tauri-drag-region
              className="truncate text-xs text-slate-500"
            >
              {session.taskListName}
            </p>
            <p data-tauri-drag-region className="truncate text-sm font-medium">
              {session.taskTitle}
            </p>
          </div>
          <TimerDisplay
            startAt={session.startAt}
            className="shrink-0 text-xl font-bold text-sky-300"
          />
          <div className="flex shrink-0 gap-1.5">
            <button
              onClick={() => void stopTask("paused")}
              title="中断"
              className="rounded-md border border-amber-500/50 px-2 py-1 text-xs text-amber-300 hover:bg-amber-500/10"
            >
              中断
            </button>
            <button
              onClick={() => void stopTask("completed")}
              title="完了"
              className="rounded-md bg-emerald-600 px-2 py-1 text-xs text-white hover:bg-emerald-500"
            >
              完了
            </button>
          </div>
        </>
      ) : (
        <span data-tauri-drag-region className="flex-1 text-sm text-slate-500">
          作業中タスクなし
        </span>
      )}
      <button
        onClick={() => void getCurrentWindow().hide()}
        title="閉じる"
        className="shrink-0 rounded-md border border-slate-700 px-2 py-1 text-xs text-slate-400 hover:bg-slate-800"
      >
        閉じる
      </button>
    </div>
  );
}
