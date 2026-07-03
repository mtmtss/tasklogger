import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import {
  dismissInterrupted,
  getInterruptedTask,
  resumeInterrupted,
  type InterruptedTask,
} from "../lib/commands";

/**
 * スリープ復帰・起動時の「再開しますか？」ダイアログ (spec §7.2, §7.3)。
 * 起動時に interrupted task を照会し、復帰時は power-resumed イベントで開く。
 */
export default function ResumeDialog() {
  const [task, setTask] = useState<InterruptedTask | null>(null);
  const [error, setError] = useState<string | null>(null);
  const queryClient = useQueryClient();

  useEffect(() => {
    getInterruptedTask().then(setTask).catch(() => {});
    const unlisten = listen<InterruptedTask>("power-resumed", (event) => {
      setTask(event.payload);
      setError(null);
    });
    return () => void unlisten.then((fn) => fn());
  }, []);

  if (!task) return null;

  const close = () => {
    setTask(null);
    setError(null);
    void queryClient.invalidateQueries();
  };

  const handleResume = () => {
    resumeInterrupted()
      .then(close)
      .catch((e) => setError(String(e)));
  };

  const handleDismiss = () => {
    dismissInterrupted().then(close).catch(close);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="w-full max-w-md rounded-xl border border-slate-700 bg-slate-900 p-6 shadow-xl">
        <h2 className="text-lg font-semibold">作業を再開しますか？</h2>
        <p className="mt-2 text-sm text-slate-400">
          中断していたタスク
          <span className="mx-1 font-medium text-slate-200">
            「{task.taskTitle}」
          </span>
          があります。
        </p>
        {error && <p className="mt-2 text-sm text-rose-300">{error}</p>}
        <div className="mt-5 flex justify-end gap-2">
          <button
            onClick={handleDismiss}
            className="rounded-md border border-slate-700 px-4 py-2 text-sm text-slate-300 hover:bg-slate-800"
          >
            今はしない
          </button>
          <button
            onClick={handleResume}
            className="rounded-md bg-sky-600 px-4 py-2 text-sm text-white hover:bg-sky-500"
          >
            再開する
          </button>
        </div>
      </div>
    </div>
  );
}
