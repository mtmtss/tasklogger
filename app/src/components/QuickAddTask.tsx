import { useEffect, useState } from "react";
import { quickAddTask } from "../lib/commands";
import { useTaskLists } from "../lib/queries";

function localDate(offsetDays: number): string {
  const d = new Date();
  d.setDate(d.getDate() + offsetDays);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** AI を通さない手動クイック追加 (AI 拡張仕様 §13.6)。Google Tasks へ直接作成する。 */
export default function QuickAddTask({
  onError,
}: {
  onError: (message: string) => void;
}) {
  const lists = (useTaskLists().data ?? []).filter(
    (l) => !l.id.startsWith("sample-"),
  );
  const [title, setTitle] = useState("");
  const [listId, setListId] = useState("");
  const [due, setDue] = useState<"today" | "tomorrow" | "none">("today");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!listId && lists.length > 0) setListId(lists[0].id);
  }, [lists, listId]);

  const submit = async () => {
    if (!title.trim() || !listId || busy) return;
    setBusy(true);
    try {
      await quickAddTask(
        listId,
        title,
        due === "none" ? null : localDate(due === "tomorrow" ? 1 : 0),
      );
      setTitle("");
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  };

  if (lists.length === 0) return null;

  return (
    <div className="flex flex-wrap items-center gap-2 rounded-lg border border-slate-800 bg-slate-900/40 px-3 py-2">
      <input
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") void submit();
        }}
        placeholder="+ タスクを直接追加"
        className="min-w-40 flex-1 bg-transparent text-sm outline-none placeholder:text-slate-500"
      />
      <select
        value={listId}
        onChange={(e) => setListId(e.target.value)}
        className="rounded-md border border-slate-700 bg-slate-900 px-2 py-1 text-xs text-slate-300"
      >
        {lists.map((l) => (
          <option key={l.id} value={l.id}>
            {l.title}
          </option>
        ))}
      </select>
      <select
        value={due}
        onChange={(e) => setDue(e.target.value as typeof due)}
        className="rounded-md border border-slate-700 bg-slate-900 px-2 py-1 text-xs text-slate-300"
      >
        <option value="today">今日</option>
        <option value="tomorrow">明日</option>
        <option value="none">期限なし</option>
      </select>
      <button
        onClick={() => void submit()}
        disabled={busy || !title.trim()}
        className="rounded-md bg-sky-600 px-3 py-1 text-xs text-white hover:bg-sky-500 disabled:opacity-40"
      >
        {busy ? "登録中…" : "追加"}
      </button>
    </div>
  );
}
