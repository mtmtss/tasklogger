import { useEffect, useState } from "react";
import type { TaskListOption } from "../types";

interface Props {
  taskLists: TaskListOption[];
  onAdd: (taskListId: string, title: string) => Promise<unknown>;
  /** 追加したタスクをそのまま開始する。開始できなかった場合も追加自体は成立する。 */
  onAddAndStart: (taskListId: string, title: string) => Promise<unknown>;
}

/** タスクをその場で追加するための、インラインのクイック追加フォーム。 */
export default function QuickAddTask({ taskLists, onAdd, onAddAndStart }: Props) {
  const [title, setTitle] = useState("");
  const [taskListId, setTaskListId] = useState("");
  // 送信中のボタン種別。null なら送信していない (二重送信防止も兼ねる)。
  const [pending, setPending] = useState<"add" | "start" | null>(null);

  useEffect(() => {
    if (!taskListId && taskLists.length > 0) {
      setTaskListId(taskLists[0].taskListId);
    }
  }, [taskLists, taskListId]);

  const canSubmit = title.trim().length > 0 && taskListId !== "" && pending === null;

  const submit = (kind: "add" | "start") => {
    if (!canSubmit) return;
    setPending(kind);
    const action = kind === "start" ? onAddAndStart : onAdd;
    action(taskListId, title.trim())
      .then(() => setTitle(""))
      .catch(() => {})
      .finally(() => setPending(null));
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    submit("add");
  };

  return (
    <form
      onSubmit={handleSubmit}
      className="flex items-center gap-2 rounded-lg border border-dashed border-slate-700 bg-slate-900/40 px-4 py-2.5"
    >
      <input
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        placeholder="タスクをその場で追加..."
        className="min-w-0 flex-1 rounded-md border border-slate-700 bg-slate-900 px-3 py-1.5 text-sm placeholder:text-slate-600 focus:border-sky-600 focus:outline-none"
      />
      <select
        value={taskListId}
        onChange={(e) => setTaskListId(e.target.value)}
        className="shrink-0 rounded-md border border-slate-700 bg-slate-900 px-2 py-1.5 text-sm focus:border-sky-600 focus:outline-none"
      >
        {taskLists.map((list) => (
          <option key={list.taskListId} value={list.taskListId}>
            {list.taskListName}
          </option>
        ))}
      </select>
      <button
        type="submit"
        disabled={!canSubmit}
        className="shrink-0 rounded-md bg-sky-600 px-3 py-1.5 text-sm text-white hover:bg-sky-500 disabled:cursor-not-allowed disabled:opacity-50"
      >
        追加
      </button>
      <button
        type="button"
        onClick={() => submit("start")}
        disabled={!canSubmit}
        className="shrink-0 rounded-md bg-pink-600 px-3 py-1.5 text-sm text-white hover:bg-pink-500 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {pending === "start" ? "開始中..." : "追加して開始"}
      </button>
    </form>
  );
}
