import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  completeTaskDirect,
  createTask,
  deleteTask,
  doItNow,
  restoreTask,
  startTask,
  stopTask,
  toggleFloatWindow,
  updateTaskDue,
} from "../lib/commands";
import { useCandidates, useDashboard, useTaskLists } from "../lib/queries";
import { formatJapaneseDate, formatMinutes } from "../lib/format";
import type { TaskItem } from "../types";
import QuickAddTask from "../components/QuickAddTask";
import RunningPanel from "../components/RunningPanel";
import SyncBadge from "../components/SyncBadge";
import TaskCard from "../components/TaskCard";
import TaskMenu from "../components/TaskMenu";

export default function TodayPage() {
  const dashboard = useDashboard();
  const candidates = useCandidates();
  const taskLists = useTaskLists();
  const queryClient = useQueryClient();
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  // 直近に削除したタスク。数秒間だけ「元に戻す」トーストを出す。
  const [deleted, setDeleted] = useState<TaskItem | null>(null);

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
    void queryClient.invalidateQueries({ queryKey: ["candidates"] });
  };

  useEffect(() => {
    if (!deleted) return;
    const id = setTimeout(() => setDeleted(null), 6000);
    return () => clearTimeout(id);
  }, [deleted]);

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
  const handleUpdateDue = (task: TaskItem, due: string | null) =>
    run(() => updateTaskDue({ taskListId: task.taskListId, taskId: task.taskId }, due));
  const handleDelete = (task: TaskItem) => {
    setError(null);
    setNotice(null);
    deleteTask({ taskListId: task.taskListId, taskId: task.taskId })
      .then(() => {
        refresh();
        setDeleted(task);
      })
      .catch((e) => setError(String(e)));
  };
  const handleUndoDelete = () => {
    if (!deleted) return;
    const task = deleted;
    setDeleted(null);
    setError(null);
    restoreTask({ taskListId: task.taskListId, taskId: task.taskId })
      .then((res) => {
        refresh();
        setNotice(
          res.recreated
            ? `「${task.title}」を復元しました。Google 側で削除済みだったため、新しいタスクとして作り直しています。`
            : null,
        );
      })
      .catch((e) => setError(String(e)));
  };
  const handleAddTask = (taskListId: string, title: string) => {
    setError(null);
    return createTask(taskListId, title)
      .then(refresh)
      .catch((e) => {
        setError(String(e));
        throw e;
      });
  };
  const handleAddAndStartTask = (taskListId: string, title: string) => {
    setError(null);
    return createTask(taskListId, title)
      .catch((e) => {
        setError(String(e));
        throw e; // 作成に失敗したときだけ、入力内容を残すため呼び出し元にも伝える
      })
      .then((task) =>
        // 実行中タスクがある場合など、開始だけ失敗しても作成は成立している。
        // 追加をなかったことにはせず、メッセージで知らせるだけに留める。
        doItNow({ taskListId: task.taskListId, taskId: task.taskId }).catch((e) =>
          setError(`タスクは追加しましたが、開始できませんでした: ${String(e)}`),
        ),
      )
      .then(refresh);
  };

  const data = dashboard.data;

  return (
    <section className="space-y-5">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold">
          今日やるリスト
          <span className="ml-3 text-sm font-normal text-slate-400">
            {data ? formatJapaneseDate(data.dateText) : ""}
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

      {notice && (
        <div className="rounded-md border border-sky-500/50 bg-sky-500/10 px-4 py-2 text-sm text-sky-200">
          {notice}
          <button
            onClick={() => setNotice(null)}
            className="ml-3 text-xs text-sky-400 underline"
          >
            閉じる
          </button>
        </div>
      )}

      {taskLists.data && taskLists.data.length > 0 && (
        <QuickAddTask
          taskLists={taskLists.data}
          onAdd={handleAddTask}
          onAddAndStart={handleAddAndStartTask}
        />
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
                onUpdateDue={handleUpdateDue}
                onDelete={handleDelete}
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
                <div className="flex shrink-0 gap-2">
                  <button
                    onClick={() =>
                      run(() =>
                        doItNow({ taskListId: task.taskListId, taskId: task.taskId }),
                      )
                    }
                    className="rounded-md bg-pink-600 px-3 py-1.5 text-sm text-white hover:bg-pink-500"
                  >
                    今すぐやる
                  </button>
                  <TaskMenu onDelete={() => handleDelete(task)} />
                </div>
              </div>
            )),
          )}
        </div>
      </div>

      {deleted && (
        <div className="fixed bottom-6 left-1/2 z-50 flex -translate-x-1/2 items-center gap-3 rounded-lg border border-rose-500/40 bg-slate-900 px-4 py-2.5 text-sm text-slate-200 shadow-xl">
          <span className="truncate">
            「{deleted.title}」を<span className="text-rose-400">削除しました</span>
          </span>
          <button
            onClick={handleUndoDelete}
            className="shrink-0 rounded-md border border-rose-500/50 bg-rose-500/15 px-3 py-1 text-xs font-medium text-rose-300 hover:bg-rose-500/25"
          >
            元に戻す
          </button>
          <button
            onClick={() => setDeleted(null)}
            aria-label="閉じる"
            className="shrink-0 text-slate-500 hover:text-slate-300"
          >
            ✕
          </button>
        </div>
      )}
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
