import { useState } from "react";
import type { ActiveSessionView } from "../types";
import TimerDisplay from "./TimerDisplay";

interface Props {
  session: ActiveSessionView;
  onStop: (action: "paused" | "completed", memo: string) => void;
}

/** 実行中タスクのパネル (spec §5.4)。 */
export default function RunningPanel({ session, onStop }: Props) {
  const [memo, setMemo] = useState("");

  return (
    <div className="rounded-xl border border-sky-500/40 bg-sky-500/5 p-4">
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0">
          <p className="text-xs text-slate-400">{session.taskListName}</p>
          <p className="truncate text-lg font-semibold">{session.taskTitle}</p>
          <p className="mt-0.5 text-xs text-slate-500">
            {new Date(session.startAt).toLocaleTimeString("ja-JP", {
              hour: "2-digit",
              minute: "2-digit",
            })}{" "}
            開始
          </p>
        </div>
        <TimerDisplay
          startAt={session.startAt}
          className="text-3xl font-bold text-sky-300"
        />
      </div>
      <div className="mt-3 flex items-center gap-2">
        <input
          value={memo}
          onChange={(e) => setMemo(e.target.value)}
          placeholder="メモ (中断/完了時に保存)"
          className="min-w-0 flex-1 rounded-md border border-slate-700 bg-slate-900 px-3 py-1.5 text-sm placeholder:text-slate-600 focus:border-sky-600 focus:outline-none"
        />
        <button
          onClick={() => onStop("paused", memo)}
          className="rounded-md border border-amber-500/50 px-3 py-1.5 text-sm text-amber-300 hover:bg-amber-500/10"
        >
          中断
        </button>
        <button
          onClick={() => onStop("completed", memo)}
          className="rounded-md bg-emerald-600 px-3 py-1.5 text-sm text-white hover:bg-emerald-500"
        >
          完了
        </button>
      </div>
    </div>
  );
}
