import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  completeTaskDirect,
  doItNow,
  startTask,
  stopTask,
  toggleFloatWindow,
} from "../lib/commands";
import { useCandidates, useDashboard } from "../lib/queries";
import { formatMinutes } from "../lib/format";
import type { TaskItem } from "../types";
import RunningPanel from "../components/RunningPanel";
import SyncBadge from "../components/SyncBadge";
import TaskCard from "../components/TaskCard";

export default function TodayPage() {
  const dashboard = useDashboard();
  const candidates = useCandidates();
  const queryClient = useQueryClient();
  const [error, setError] = useState<string | null>(null);

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
    void queryClient.invalidateQueries({ queryKey: ["candidates"] });
  };

  const run = (fn: () => Promise<unknown>) => {
    setError(null);
    fn()
      .then(refresh)
      .catch((e) => setError(String(e)));
  };

  const handleStart = (task: TaskItem) =>
    run(() => startTask({ taskListId: task.taskListId, taskId: task.taskId }));
  const handlePause = () => run(() => stopTask("paused"));
  const handleComplete = (task: TaskItem) =>
    task.appStatus === "running"
      ? run(() => stopTask("completed"))
      : run(() =>
          completeTaskDirect({ taskListId: task.taskListId, taskId: task.taskId }),
        );

  const data = dashboard.data;

  return (
    <section className="space-y-5">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold">
          今日やるリスト
          <span className="ml-3 text-sm font-normal text-slate-400">
            {data?.dateText}
          </span>
        </h1>
        <div className="flex items-center gap-3">
          <SyncBadge onError={setError} />
          <button
            onClick={() => void toggleFloatWindow()}
            className="rounded-md border border-slate-700 px-3 py-1.5 text-sm text-slate-300 hover:bg-slate-800"
          >
            フロート表示切替
          </button>
        </div>
      </div>

      {error && (
        <div className="rounded-md border border-rose-500/50 bg-rose-500/10 px-4 py-2 text-sm text-rose-300">
          {error}
          <button
            onClick={() => setError(null)}
            className="ml-3 text-xs text-rose-400 underline"
          >
            閉じる
          </button>
        </div>
      )}

      {data && (
        <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
          <SummaryCard
            label="今日の作業時間"
            value={formatMinutes(data.analytics.totalSeconds)}
          />
          <SummaryCard
            label="セッション数"
            value={`${data.analytics.totalSessions} 回`}
          />
          <SummaryCard
            label="中断中タスク"
            value={`${data.analytics.pausedTaskCount} 件`}
          />
          <SummaryCard
            label="今日のタスク"
            value={`${data.taskGroups.reduce((n, g) => n + g.tasks.length, 0)} 件`}
          />
        </div>
      )}

      {data?.activeSession && (
        <RunningPanel
          session={data.activeSession}
          onStop={(action, memo) => run(() => stopTask(action, memo))}
        />
      )}

      {data?.taskGroups.length === 0 && (
        <p className="rounded-lg border border-dashed border-slate-800 px-4 py-8 text-center text-sm text-slate-500">
          今日期限のタスクはありません。下の候補から「今すぐやる」で始められます。
        </p>
      )}

      {data?.taskGroups.map((group) => (
        <div key={group.taskListId}>
          <h2 className="mb-2 text-sm font-semibold text-slate-400">
            {group.taskListName}
          </h2>
          <div className="space-y-2">
            {group.tasks.map((task) => (
              <TaskCard
                key={task.taskId}
                task={task}
                onStart={handleStart}
                onPause={handlePause}
                onComplete={handleComplete}
              />
            ))}
          </div>
        </div>
      ))}

      {data && data.analytics.byTaskList.length > 0 && (
        <div>
          <h2 className="mb-2 text-sm font-semibold text-slate-400">
            タスクリスト別 (今日)
          </h2>
          <div className="space-y-1.5">
            {data.analytics.byTaskList.map((item) => {
              const ratio =
                data.analytics.totalSeconds > 0
                  ? (item.totalSeconds / data.analytics.totalSeconds) * 100
                  : 0;
              return (
                <div
                  key={item.taskListId}
                  className="flex items-center gap-3 text-sm"
                >
                  <span className="w-32 truncate text-slate-300">
                    {item.taskListName}
                  </span>
                  <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-slate-800">
                    <div
                      className="h-full rounded-full bg-sky-500"
                      style={{ width: `${ratio}%` }}
                    />
                  </div>
                  <span className="w-20 text-right text-slate-400">
                    {formatMinutes(item.totalSeconds)}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      <div>
        <h2 className="mb-2 text-sm font-semibold text-slate-400">候補タスク</h2>
        {candidates.data?.length === 0 && (
          <p className="text-sm text-slate-500">候補はありません。</p>
        )}
        <div className="space-y-2">
          {candidates.data?.flatMap((group) =>
            group.tasks.map((task) => (
              <div
                key={task.taskId}
                className="flex items-center justify-between gap-3 rounded-lg border border-slate-800 bg-slate-900/40 px-4 py-2.5"
              >
                <div className="min-w-0">
                  <span className="mr-2 text-xs text-slate-500">
                    {group.taskListName}
                  </span>
                  <span className="truncate text-sm">{task.title}</span>
                </div>
                <button
                  onClick={() =>
                    run(() =>
                      doItNow({ taskListId: task.taskListId, taskId: task.taskId }),
                    )
                  }
                  className="shrink-0 rounded-md bg-pink-600 px-3 py-1.5 text-sm text-white hover:bg-pink-500"
                >
                  今すぐやる
                </button>
              </div>
            )),
          )}
        </div>
      </div>
    </section>
  );
}

function SummaryCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-slate-800 bg-slate-900/60 px-4 py-3">
      <p className="text-xs text-slate-500">{label}</p>
      <p className="mt-1 text-lg font-semibold">{value}</p>
    </div>
  );
}
