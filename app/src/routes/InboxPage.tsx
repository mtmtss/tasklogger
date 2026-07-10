import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  addCapture,
  classifyCapture,
  dismissCapture,
  getAiStatus,
  registerCaptureItem,
  type Capture,
  type CaptureItem,
  type CaptureKind,
  type TaskListOption,
} from "../lib/commands";
import { useCaptures, useTaskLists } from "../lib/queries";

const KIND_LABELS: Record<CaptureKind, string> = {
  TASK: "タスク",
  RESEARCH_IDEA: "研究アイデア",
  LIFE_ADMIN: "生活・事務",
  SOMEDAY: "Someday",
  UNCLEAR: "要確認",
};

const KIND_STYLES: Record<CaptureKind, string> = {
  TASK: "bg-sky-500/15 text-sky-300 border-sky-500/40",
  RESEARCH_IDEA: "bg-violet-500/15 text-violet-300 border-violet-500/40",
  LIFE_ADMIN: "bg-emerald-500/15 text-emerald-300 border-emerald-500/40",
  SOMEDAY: "bg-slate-500/15 text-slate-300 border-slate-500/40",
  UNCLEAR: "bg-amber-500/15 text-amber-300 border-amber-500/40",
};

/** 種別ごとの専用リスト (無ければ登録時に自動作成, 仕様 §13.5)。 */
const SPECIAL_LISTS: Partial<Record<CaptureKind, string>> = {
  RESEARCH_IDEA: "研究アイデア",
  SOMEDAY: "Someday",
};

/** 登録先 select の値: 既存リストは "id:<id>"、自動作成は "name:<リスト名>"。 */
function defaultTarget(item: CaptureItem, lists: TaskListOption[]): string {
  if (item.listName) {
    const match = lists.find((l) => l.title === item.listName);
    if (match) return `id:${match.id}`;
  }
  const special = SPECIAL_LISTS[item.kind];
  if (special) {
    const match = lists.find((l) => l.title === special);
    return match ? `id:${match.id}` : `name:${special}`;
  }
  const first = lists.find((l) => !l.id.startsWith("sample-"));
  return first ? `id:${first.id}` : "";
}

function targetToInput(target: string): {
  listId: string | null;
  listName: string | null;
} {
  if (target.startsWith("id:")) return { listId: target.slice(3), listName: null };
  if (target.startsWith("name:"))
    return { listId: null, listName: target.slice(5) };
  return { listId: null, listName: null };
}

/** Inbox (AI 拡張仕様 §13.5): キャプチャの確認・編集・Google Tasks への登録。 */
export default function InboxPage() {
  const captures = useCaptures();
  const lists = (useTaskLists().data ?? []).filter(
    (l) => !l.id.startsWith("sample-"),
  );
  const aiStatus = useQuery({ queryKey: ["aiStatus"], queryFn: getAiStatus });
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [bulkBusy, setBulkBusy] = useState(false);

  const items = captures.data ?? [];
  const bulkTargets = items.flatMap((capture) =>
    capture.status === "classified"
      ? (capture.aiResult?.items ?? [])
          .map((item, index) => ({ capture, item, index }))
          .filter(
            ({ item }) => !item.registeredTaskId && item.kind !== "UNCLEAR",
          )
      : [],
  );

  const submitCapture = async () => {
    if (!text.trim()) return;
    try {
      await addCapture(text);
      setText("");
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  /** 提案どおりまとめて登録 (要確認は除く)。編集内容ではなく AI 提案の既定値を使う。 */
  const registerAll = async () => {
    setBulkBusy(true);
    setError(null);
    try {
      for (const { capture, item, index } of bulkTargets) {
        const target = targetToInput(defaultTarget(item, lists));
        await registerCaptureItem(capture.id, {
          title: item.title,
          due: item.due ?? null,
          firstStep: item.firstStep ?? null,
          note: item.note ?? null,
          itemIndex: index,
          ...target,
        });
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBulkBusy(false);
    }
  };

  return (
    <section className="space-y-5">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold">
          Inbox
          <span className="ml-3 text-sm font-normal text-slate-400">
            思いつきを放り込み、AI の提案を承認して登録する
          </span>
        </h1>
        {bulkTargets.length > 0 && (
          <button
            onClick={() => void registerAll()}
            disabled={bulkBusy}
            className="rounded-md bg-sky-600 px-3 py-1.5 text-sm text-white hover:bg-sky-500 disabled:opacity-40"
          >
            {bulkBusy
              ? "登録中…"
              : `提案どおりまとめて登録 (${bulkTargets.length}件)`}
          </button>
        )}
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

      <div className="flex items-start gap-2 rounded-lg border border-slate-800 bg-slate-900/40 px-3 py-2">
        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              void submitCapture();
            }
          }}
          rows={2}
          placeholder="タスク・アイデア・メモを何でも (Enter で保存 / どこからでも ホットキー でこの入力を開けます)"
          className="min-w-0 flex-1 resize-none bg-transparent text-sm outline-none placeholder:text-slate-500"
        />
        <button
          onClick={() => void submitCapture()}
          disabled={!text.trim()}
          className="shrink-0 rounded-md bg-sky-600 px-3 py-1.5 text-sm text-white hover:bg-sky-500 disabled:opacity-40"
        >
          保存
        </button>
      </div>

      {items.length === 0 && (
        <p className="rounded-lg border border-dashed border-slate-800 px-4 py-8 text-center text-sm text-slate-500">
          未処理のキャプチャはありません。
        </p>
      )}

      <div className="space-y-3">
        {items.map((capture) => (
          <CaptureCard
            key={capture.id}
            capture={capture}
            lists={lists}
            aiConfigured={aiStatus.data?.configured ?? false}
            onError={setError}
          />
        ))}
      </div>
    </section>
  );
}

function CaptureCard({
  capture,
  lists,
  aiConfigured,
  onError,
}: {
  capture: Capture;
  lists: TaskListOption[];
  aiConfigured: boolean;
  onError: (message: string) => void;
}) {
  const [busy, setBusy] = useState(false);
  const items = capture.aiResult?.items ?? [];

  const retry = async () => {
    setBusy(true);
    try {
      await classifyCapture(capture.id);
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const dismiss = async () => {
    try {
      await dismissCapture(capture.id);
    } catch (e) {
      onError(String(e));
    }
  };

  return (
    <div className="rounded-lg border border-slate-800 bg-slate-900/40">
      <div className="flex items-start justify-between gap-3 px-4 py-3">
        <div className="min-w-0">
          <p className="whitespace-pre-wrap text-sm text-slate-200">
            {capture.text}
          </p>
          <p className="mt-1 text-xs text-slate-500">
            {capture.createdAt.slice(0, 10)}{" "}
            {capture.status === "pending" &&
              (aiConfigured ? "· AI 分類待ち" : "· AI 未設定 (設定ページでキーを入力すると自動分類されます)")}
          </p>
        </div>
        <div className="flex shrink-0 gap-1.5">
          {capture.status === "pending" && aiConfigured && (
            <button
              onClick={() => void retry()}
              disabled={busy}
              className="rounded-md border border-slate-700 px-2 py-1 text-xs text-slate-300 hover:bg-slate-800 disabled:opacity-40"
            >
              {busy ? "分類中…" : "AI 分類"}
            </button>
          )}
          <button
            onClick={() => void dismiss()}
            className="rounded-md border border-slate-700 px-2 py-1 text-xs text-slate-400 hover:bg-slate-800"
          >
            破棄
          </button>
        </div>
      </div>
      {items.length > 0 && (
        <div className="divide-y divide-slate-800/60 border-t border-slate-800">
          {items.map((item, index) => (
            <ItemRow
              key={`${capture.id}-${index}`}
              captureId={capture.id}
              item={item}
              index={index}
              lists={lists}
              onError={onError}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function ItemRow({
  captureId,
  item,
  index,
  lists,
  onError,
}: {
  captureId: string;
  item: CaptureItem;
  index: number;
  lists: TaskListOption[];
  onError: (message: string) => void;
}) {
  const [kind, setKind] = useState<CaptureKind>(item.kind);
  const [title, setTitle] = useState(item.title);
  const [target, setTarget] = useState(() => defaultTarget(item, lists));
  const [due, setDue] = useState(item.due ?? "");
  const [busy, setBusy] = useState(false);
  const registered = !!item.registeredTaskId;

  const changeKind = (next: CaptureKind) => {
    setKind(next);
    setTarget(defaultTarget({ ...item, kind: next, listName: null }, lists));
  };

  const register = async () => {
    setBusy(true);
    try {
      await registerCaptureItem(captureId, {
        title,
        due: due || null,
        firstStep: item.firstStep ?? null,
        note: item.note ?? null,
        itemIndex: index,
        ...targetToInput(target),
      });
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  };

  // 登録先の選択肢: 既存リスト + 未作成の専用リスト
  const targetOptions: { value: string; label: string }[] = [
    ...lists.map((l) => ({ value: `id:${l.id}`, label: l.title })),
    ...Object.values(SPECIAL_LISTS)
      .filter((name) => !lists.some((l) => l.title === name))
      .map((name) => ({ value: `name:${name}`, label: `${name} (新規作成)` })),
  ];

  return (
    <div className="flex flex-wrap items-center gap-2 px-4 py-2.5">
      <select
        value={kind}
        onChange={(e) => changeKind(e.target.value as CaptureKind)}
        disabled={registered}
        className={`rounded-md border px-1.5 py-0.5 text-xs ${KIND_STYLES[kind]} bg-slate-900 disabled:opacity-60`}
      >
        {(Object.keys(KIND_LABELS) as CaptureKind[]).map((k) => (
          <option key={k} value={k} className="bg-slate-900 text-slate-200">
            {KIND_LABELS[k]}
          </option>
        ))}
      </select>
      <input
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        disabled={registered}
        className="min-w-40 flex-1 rounded-md bg-slate-800/60 px-2 py-1 text-sm outline-none focus:ring-1 focus:ring-sky-600 disabled:opacity-60"
      />
      <select
        value={target}
        onChange={(e) => setTarget(e.target.value)}
        disabled={registered}
        className="rounded-md border border-slate-700 bg-slate-900 px-2 py-1 text-xs text-slate-300 disabled:opacity-60"
      >
        {targetOptions.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <input
        type="date"
        value={due}
        onChange={(e) => setDue(e.target.value)}
        disabled={registered}
        className="rounded-md border border-slate-700 bg-slate-900 px-2 py-1 text-xs text-slate-300 disabled:opacity-60"
      />
      {registered ? (
        <span className="shrink-0 text-xs text-emerald-400">✓ 登録済み</span>
      ) : (
        <button
          onClick={() => void register()}
          disabled={busy || !title.trim() || !target}
          className="shrink-0 rounded-md bg-sky-600 px-3 py-1 text-xs text-white hover:bg-sky-500 disabled:opacity-40"
        >
          {busy ? "登録中…" : "登録"}
        </button>
      )}
      {item.firstStep && (
        <p className="w-full text-xs text-slate-500">
          最初の一手: {item.firstStep}
        </p>
      )}
    </div>
  );
}
