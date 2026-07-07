import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  generateDailyReview,
  getAiStatus,
  getDailyReview,
  scheduleForToday,
  type ReviewTomorrowItem,
} from "../lib/commands";

/**
 * AI 拡張「日次レビュー」カード (docs/ai-extension-specification.md §10 第2弾)。
 * API キー未設定なら何も表示しない (既存 UI への影響ゼロ)。
 */
export default function DailyReviewCard({
  onError,
}: {
  onError: (message: string) => void;
}) {
  const aiStatus = useQuery({ queryKey: ["aiStatus"], queryFn: getAiStatus });
  const [generating, setGenerating] = useState(false);
  const [collapsed, setCollapsed] = useState(true);
  const queryClient = useQueryClient();

  // 生成済みレビューはクエリキャッシュに保持 (プランと同様、再訪・再起動でも復元)。
  const reviewQuery = useQuery({
    queryKey: ["dailyReview"],
    queryFn: getDailyReview,
    enabled: aiStatus.data?.configured ?? false,
  });
  const review = reviewQuery.data ?? null;

  if (!aiStatus.data?.configured) return null;

  const handleGenerate = () => {
    setGenerating(true);
    generateDailyReview()
      .then((stored) => {
        queryClient.setQueryData(["dailyReview"], stored);
        setCollapsed(false);
      })
      .catch((e) => onError(String(e)))
      .finally(() => setGenerating(false));
  };

  const handleScheduleTomorrow = (item: ReviewTomorrowItem) => {
    if (!item.taskListId || !item.taskId) return;
    scheduleForToday({ taskListId: item.taskListId, taskId: item.taskId })
      .then(() => void queryClient.invalidateQueries())
      .catch((e) => onError(String(e)));
  };

  return (
    <div className="rounded-xl border border-indigo-500/40 bg-indigo-500/5 p-4">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-indigo-300">
          今日の振り返り
          {review && (
            <span className="ml-2 text-xs font-normal text-slate-500">
              {new Date(review.generatedAt).toLocaleTimeString("ja-JP", {
                hour: "2-digit",
                minute: "2-digit",
              })}{" "}
              生成
            </span>
          )}
        </h2>
        <div className="flex items-center gap-2">
          <button
            onClick={handleGenerate}
            disabled={generating}
            className="rounded-md bg-indigo-600 px-3 py-1.5 text-xs text-white hover:bg-indigo-500 disabled:opacity-50"
          >
            {generating ? "振り返り中…" : review ? "振り返り直す" : "1日を振り返る"}
          </button>
          {review && (
            <button
              onClick={() => setCollapsed(!collapsed)}
              className="text-xs text-slate-500 hover:text-slate-300"
            >
              {collapsed ? "開く" : "たたむ"}
            </button>
          )}
        </div>
      </div>

      {generating && (
        <p className="mt-2 text-xs text-slate-500">
          計画と実績を突合しています (30秒〜1分程度)…
        </p>
      )}

      {review && !collapsed && !generating && (
        <div className="mt-4 space-y-4">
          {review.review.done.length > 0 && (
            <div>
              <h3 className="mb-1.5 text-xs font-semibold text-emerald-300">
                できたこと
              </h3>
              <ul className="space-y-1 text-sm text-slate-300">
                {review.review.done.map((d, i) => (
                  <li key={i} className="flex gap-2">
                    <span className="text-emerald-400">✓</span>
                    <span>{d}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {review.review.incomplete.length > 0 && (
            <div>
              <h3 className="mb-1.5 text-xs font-semibold text-amber-300">
                未完了と理由
              </h3>
              <ul className="space-y-1 text-sm">
                {review.review.incomplete.map((item, i) => (
                  <li key={i} className="text-slate-400">
                    <span className="text-slate-300">{item.title}</span> —{" "}
                    {item.reason}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {review.review.tomorrow.length > 0 && (
            <div>
              <h3 className="mb-1.5 text-xs font-semibold text-sky-300">
                明日に回す候補
              </h3>
              <ul className="space-y-1.5">
                {review.review.tomorrow.map((item, i) => (
                  <li
                    key={i}
                    className="flex items-start justify-between gap-3 rounded-md border border-slate-800 bg-slate-900/50 px-3 py-2"
                  >
                    <div className="min-w-0">
                      <p className="text-sm text-slate-200">{item.title}</p>
                      <p className="mt-0.5 text-xs text-slate-500">
                        {item.reason}
                      </p>
                    </div>
                    {item.taskListId && item.taskId && (
                      <button
                        onClick={() => handleScheduleTomorrow(item)}
                        className="shrink-0 rounded-md border border-slate-700 px-2.5 py-1 text-xs text-slate-300 hover:bg-slate-800"
                      >
                        今日やるに入れる
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {review.review.research_progress && (
            <div>
              <h3 className="mb-1 text-xs font-semibold text-violet-300">
                研究・制作の前進
              </h3>
              <p className="text-sm text-slate-300">
                {review.review.research_progress}
              </p>
            </div>
          )}

          <p className="rounded-md bg-slate-900/60 px-3 py-2 text-xs leading-relaxed text-slate-400">
            {review.review.summary}
          </p>
        </div>
      )}
    </div>
  );
}
