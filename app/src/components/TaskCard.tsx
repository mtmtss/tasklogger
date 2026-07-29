import { useState } from "react";
import type { TaskItem } from "../types";
import { formatMinutes } from "../lib/format";
import StatusChip from "./StatusChip";
import DueDatePrompt from "./DueDatePrompt";
import TaskMenu from "./TaskMenu";

interface Props {
  task: TaskItem;
  onStart: (task: TaskItem) => void;
  onPause: () => void;
  onComplete: (task: TaskItem) => void;
  onUpdateDue: (task: TaskItem, due: string | null) => void;
  onDelete: (task: TaskItem) => void;
}

/** 今日リストの 1 タスク。状態に応じたボタンを出す (spec §5.1)。 */
export default function TaskCard({
  task,
  onStart,
  onPause,
  onComplete,
  onUpdateDue,
  onDelete,
}: Props) {
  const isRunning = task.appStatus === "running";
  const isCompleted = task.appStatus === "completed";
  const [showDuePrompt, setShowDuePrompt] = useState(false);

  return (
    <div
      className={`rounded-lg border px-4 py-3 ${
        isRunning
          ? "border-sky-500/50 bg-sky-500/5"
          : isCompleted
            ? "border-slate-800 bg-slate-900/40 opacity-60"
            : "border-slate-800 bg-slate-900/60"
      }`}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <StatusChip status={task.appStatus} />
            {task.isOverdue && !isCompleted && (
              <span className="inline-block rounded-full bg-rose-500/15 px-2 py-0.5 text-xs font-medium text-rose-300">
                期限切れ
              </span>
            )}
            <span
              className={`truncate font-medium ${isCompleted ? "line-through text-slate-500" : ""}`}
            >
              {task.title}
            </span>
          </div>
          {task.notes && (
            <p className="mt-1 truncate text-xs text-slate-500">{task.notes}</p>
          )}
          {task.todayDurationSeconds > 0 && (
            <p className="mt-1 text-xs text-slate-400">
              本日 {formatMinutes(task.todayDurationSeconds)}
            </p>
          )}
        </div>
        <div className="flex shrink-0 gap-2">
          {!isCompleted && (
            <>
              {task.appStatus === "not_started" && (
                <>
                  <ActionButton primary onClick={() => onStart(task)}>
                    開始
                  </ActionButton>
                  <ActionButton onClick={() => onComplete(task)}>完了</ActionButton>
                </>
              )}
              {task.appStatus === "running" && (
                <>
                  <ActionButton onClick={onPause}>中断</ActionButton>
                  <ActionButton primary onClick={() => onComplete(task)}>
                    完了
                  </ActionButton>
                </>
              )}
              {task.appStatus === "paused" && (
                <>
                  <ActionButton primary onClick={() => onStart(task)}>
                    再開
                  </ActionButton>
                  <ActionButton onClick={() => onComplete(task)}>完了</ActionButton>
                </>
              )}
              {task.isOverdue && (
                <ActionButton onClick={() => setShowDuePrompt(true)}>
                  期限を変更
                </ActionButton>
              )}
            </>
          )}
          {!isRunning && (
            <TaskMenu
              onDelete={() => {
                if (window.confirm(`「${task.title}」を削除しますか？`)) {
                  onDelete(task);
                }
              }}
            />
          )}
        </div>
      </div>
      {showDuePrompt && (
        <DueDatePrompt
          taskTitle={task.title}
          currentDue={task.due}
          onCancel={() => setShowDuePrompt(false)}
          onConfirm={(due) => {
            setShowDuePrompt(false);
            onUpdateDue(task, due);
          }}
        />
      )}
    </div>
  );
}

function ActionButton({
  primary,
  onClick,
  children,
}: {
  primary?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={`rounded-md px-3 py-1.5 text-sm transition-colors ${
        primary
          ? "bg-sky-600 text-white hover:bg-sky-500"
          : "border border-slate-700 text-slate-300 hover:bg-slate-800"
      }`}
    >
      {children}
    </button>
  );
}
