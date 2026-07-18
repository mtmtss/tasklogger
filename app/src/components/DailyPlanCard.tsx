import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  doItNow,
  generateDailyPlan,
  getAiStatus,
  getDailyPlan,
  scheduleForToday,
  type PlanNotTodayItem,
  type PlanTaskItem,
  type StoredPlan,
} from "../lib/commands";

/**
 * AI 拡張「今日の作戦」カード (docs/ai-extension-specification.md §3.4)。
 * API キー未設定なら何も表示しない (既存 UI への影響ゼロ)。
 */
export default function DailyPlanCard({
  onError,
}: {
  onError: (message: string) => void;
}) {
  const aiStatus = useQuery({ queryKey: ["aiStatus"], queryFn: getAiStatus });
  const [plan, setPlan] = useState<StoredPlan | null>(null);
  const [note, setNote] = useState("");
  const [generating, setGenerating] = useState(false);
  const [open, setOpen] = useState(false);
  const queryClient = useQueryClient();

  useEffect(() => {
    getDailyPlan().then(setPlan).catch(() => {});
  }, []);

  if (!aiStatus.data?.configured) return null;

  const handleGenerate = () => {
    setGenerating(true);
    generateDailyPlan(note)
      .then(setPlan)
      .catch((e) => onError(String(e)))
      .finally(() => setGenerating(false));
  };

  const refresh = () => void queryClient.invalidateQueries();

  const act = (fn: () => Promise<unknown>) => {
    fn()
      .then(refresh)
      .catch((e) => onError(String(e)));
  };

  return (
    <>
      <button
        onClick={() => setOpen(true)}
        className="flex items-center gap-2 rounded-md border border-violet-500/40 bg-violet-500/5 px-3 py-1.5 text-sm text-violet-300 hover:bg-violet-500/10"
      >
        今日の作戦
        {plan && <span className="h-1.5 w-1.5 rounded-full bg-violet-400" />}
      </button>

      {open && (
        <div
          onClick={() => setOpen(false)}
          className="fixed inset-0 z-40 bg-black/50"
        />
      )}
      <div
        className={`fixed inset-y-0 right-0 z-50 flex w-full max-w-md transform flex-col border-l border-violet-500/40 bg-slate-950 shadow-xl transition-transform duration-300 ease-in-out ${
          open ? "translate-x-0" : "translate-x-full"
        }`}
      >
        <div className="flex items-center justify-between border-b border-slate-800 p-4">
          <h2 className="text-sm font-semibold text-violet-300">
            今日の作戦
            {plan && (
              <span className="ml-2 text-xs font-normal text-slate-500">
                {new Date(plan.generatedAt).toLocaleTimeString("ja-JP", {
                  hour: "2-digit",
                  minute: "2-digit",
                })}{" "}
                生成
              </span>
            )}
          </h2>
          <button
            onClick={() => setOpen(false)}
            className="text-slate-500 hover:text-slate-300"
            aria-label="閉じる"
          >
            ✕
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-4">
          <div className="flex items-center gap-2">
            <input
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder="今日の状態 (例: 午後から外出 / 気力低め)。Win+H で音声入力可"
              className="min-w-0 flex-1 rounded-md border border-slate-700 bg-slate-950 px-3 py-1.5 text-sm placeholder:text-slate-600 focus:border-violet-500 focus:outline-none"
            />
            <button
              onClick={handleGenerate}
              disabled={generating}
              className="shrink-0 rounded-md bg-violet-600 px-3 py-1.5 text-sm text-white hover:bg-violet-500 disabled:opacity-50"
            >
              {generating ? "考え中…" : plan ? "作戦を立て直す" : "作戦を立てる"}
            </button>
          </div>
          {generating && (
            <p className="mt-2 text-xs text-slate-500">
              実績データを踏まえて考えています (30秒〜1分程度)…
            </p>
          )}

          {plan && !generating && (
            <div className="mt-4 space-y-4">
              <PlanSection
                title="必ずやる"
                accent="text-rose-300"
                items={plan.plan.must_do}
                onAct={act}
              />
              <PlanSection
                title="できればやる"
                accent="text-sky-300"
                items={plan.plan.if_possible}
                onAct={act}
              />
              <PlanSection
                title="5分でできる"
                accent="text-emerald-300"
                items={plan.plan.five_minute}
                onAct={act}
              />

              {plan.plan.not_today.length > 0 && (
                <div>
                  <h3 className="mb-1.5 text-xs font-semibold text-slate-500">
                    今日はやらない (安心して退避)
                  </h3>
                  <ul className="space-y-1">
                    {plan.plan.not_today.map((item, i) => (
                      <NotTodayRow key={i} item={item} />
                    ))}
                  </ul>
                </div>
              )}

              <p className="rounded-md bg-slate-900/60 px-3 py-2 text-xs leading-relaxed text-slate-400">
                {plan.plan.advice}
              </p>
            </div>
          )}
        </div>
      </div>
    </>
  );
}

function PlanSection({
  title,
  accent,
  items,
  onAct,
}: {
  title: string;
  accent: string;
  items: PlanTaskItem[];
  onAct: (fn: () => Promise<unknown>) => void;
}) {
  if (items.length === 0) return null;
  return (
    <div>
      <h3 className={`mb-1.5 text-xs font-semibold ${accent}`}>{title}</h3>
      <ul className="space-y-1.5">
        {items.map((item, i) => (
          <li
            key={i}
            className="rounded-md border border-slate-800 bg-slate-900/50 px-3 py-2"
          >
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="text-sm text-slate-200">
                  {item.title}
                  <span className="ml-2 text-xs text-slate-500">
                    約{item.estimatedMinutes}分
                  </span>
                </p>
                <p className="mt-0.5 text-xs text-violet-300/90">
                  最初の一手: {item.firstStep}
                </p>
                <p className="mt-0.5 text-xs text-slate-500">{item.reason}</p>
              </div>
              {item.taskListId && item.taskId && (
                <div className="flex shrink-0 gap-1.5">
                  <button
                    onClick={() =>
                      // due=today 化 + 開始。既に today のタスクにも冪等 (due 再設定のみ)。
                      // 実行中タスクがある場合は既存のブロックエラーがそのまま表示される
                      onAct(() =>
                        doItNow({
                          taskListId: item.taskListId!,
                          taskId: item.taskId!,
                        }),
                      )
                    }
                    className="rounded-md bg-pink-600 px-2.5 py-1 text-xs text-white hover:bg-pink-500"
                  >
                    今すぐやる
                  </button>
                  <button
                    onClick={() =>
                      onAct(() =>
                        scheduleForToday({
                          taskListId: item.taskListId!,
                          taskId: item.taskId!,
                        }),
                      )
                    }
                    className="rounded-md border border-slate-700 px-2.5 py-1 text-xs text-slate-300 hover:bg-slate-800"
                  >
                    今日やる
                  </button>
                </div>
              )}
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}

function NotTodayRow({ item }: { item: PlanNotTodayItem }) {
  return (
    <li className="px-3 py-1 text-xs text-slate-500">
      <span className="text-slate-400">{item.title}</span> — {item.reason}
    </li>
  );
}
