import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { getArchiveAnalytics } from "../lib/commands";
import { formatMinutes } from "../lib/format";

function dateText(date: Date): string {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

function daysAgo(days: number): string {
  const date = new Date();
  date.setDate(date.getDate() - days);
  return dateText(date);
}

const PRESETS = [
  { label: "7日", days: 6 },
  { label: "30日", days: 29 },
  { label: "90日", days: 89 },
];

export default function ArchivePage() {
  const [startDate, setStartDate] = useState(() => daysAgo(29));
  const [endDate, setEndDate] = useState(() => dateText(new Date()));
  const [taskLimit, setTaskLimit] = useState(20);

  const { data, error } = useQuery({
    queryKey: ["archive", startDate, endDate],
    queryFn: () => getArchiveAnalytics(startDate, endDate),
  });

  return (
    <section className="space-y-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-xl font-bold">アーカイブ</h1>
        <div className="flex flex-wrap items-center gap-2 text-sm">
          <input
            type="date"
            value={startDate}
            onChange={(e) => setStartDate(e.target.value)}
            className="rounded-md border border-slate-700 bg-slate-900 px-2 py-1.5 text-sm [color-scheme:dark]"
          />
          <span className="text-slate-500">〜</span>
          <input
            type="date"
            value={endDate}
            onChange={(e) => setEndDate(e.target.value)}
            className="rounded-md border border-slate-700 bg-slate-900 px-2 py-1.5 text-sm [color-scheme:dark]"
          />
          {PRESETS.map((preset) => (
            <button
              key={preset.label}
              onClick={() => {
                setStartDate(daysAgo(preset.days));
                setEndDate(dateText(new Date()));
              }}
              className="rounded-md border border-slate-700 px-2.5 py-1.5 text-xs text-slate-300 hover:bg-slate-800"
            >
              {preset.label}
            </button>
          ))}
        </div>
      </div>

      {error != null && (
        <p className="rounded-md border border-rose-500/50 bg-rose-500/10 px-4 py-2 text-sm text-rose-300">
          {String(error)}
        </p>
      )}

      {data && (
        <>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-5">
            <SummaryCard label="合計時間" value={formatMinutes(data.totalSeconds)} />
            <SummaryCard label="日平均" value={`${data.averageMinutesPerDay}分`} />
            <SummaryCard label="稼働日数" value={`${data.activeDays}日`} />
            <SummaryCard label="セッション" value={`${data.totalSessions}回`} />
            <SummaryCard label="完了" value={`${data.completedSessions}件`} />
          </div>

          <div className="rounded-lg border border-slate-800 bg-slate-900/60 p-4">
            <h2 className="mb-3 text-sm font-semibold text-slate-400">日別推移</h2>
            <ResponsiveContainer width="100%" height={220}>
              <BarChart data={data.byDate} margin={{ top: 5, right: 5, left: -20, bottom: 0 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" />
                <XAxis
                  dataKey="date"
                  tickFormatter={(d: string) => d.slice(5).replace("-", "/")}
                  tick={{ fill: "#64748b", fontSize: 11 }}
                  interval="preserveStartEnd"
                />
                <YAxis
                  tick={{ fill: "#64748b", fontSize: 11 }}
                  tickFormatter={(v: number) => `${v}分`}
                />
                <Tooltip
                  formatter={(value) => [`${value}分`, "作業時間"]}
                  labelFormatter={(label) => String(label)}
                  contentStyle={{
                    background: "#0f172a",
                    border: "1px solid #334155",
                    borderRadius: 8,
                    color: "#e2e8f0",
                  }}
                />
                <Bar dataKey="totalMinutes" fill="#0ea5e9" radius={[3, 3, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </div>

          <div className="grid gap-5 lg:grid-cols-2">
            <div className="rounded-lg border border-slate-800 bg-slate-900/60 p-4">
              <h2 className="mb-3 text-sm font-semibold text-slate-400">
                タスクリスト別
              </h2>
              {data.byTaskList.length === 0 && (
                <p className="text-sm text-slate-500">記録がありません。</p>
              )}
              <div className="space-y-2">
                {data.byTaskList.map((item) => {
                  const ratio =
                    data.totalSeconds > 0
                      ? (item.totalSeconds / data.totalSeconds) * 100
                      : 0;
                  return (
                    <div key={item.taskListId} className="text-sm">
                      <div className="flex justify-between">
                        <span className="truncate text-slate-300">
                          {item.taskListName}
                        </span>
                        <span className="shrink-0 text-slate-400">
                          {formatMinutes(item.totalSeconds)} / {item.sessionCount}回 /
                          完了{item.completedCount}
                        </span>
                      </div>
                      <div className="mt-1 h-1.5 overflow-hidden rounded-full bg-slate-800">
                        <div
                          className="h-full rounded-full bg-sky-500"
                          style={{ width: `${ratio}%` }}
                        />
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>

            <div className="rounded-lg border border-slate-800 bg-slate-900/60 p-4">
              <div className="mb-3 flex items-center justify-between">
                <h2 className="text-sm font-semibold text-slate-400">タスク別</h2>
                <select
                  value={taskLimit}
                  onChange={(e) => setTaskLimit(Number(e.target.value))}
                  className="rounded-md border border-slate-700 bg-slate-900 px-2 py-1 text-xs text-slate-300"
                >
                  <option value={10}>上位10件</option>
                  <option value={20}>上位20件</option>
                  <option value={50}>上位50件</option>
                  <option value={100000}>すべて</option>
                </select>
              </div>
              {data.byTask.length === 0 && (
                <p className="text-sm text-slate-500">記録がありません。</p>
              )}
              <div className="space-y-1.5">
                {data.byTask.slice(0, taskLimit).map((item) => (
                  <div
                    key={`${item.taskListId}-${item.taskId}`}
                    className="flex items-center justify-between gap-3 rounded-md border border-slate-800/60 px-3 py-2 text-sm"
                  >
                    <div className="min-w-0">
                      <p className="truncate text-slate-200">{item.taskTitle}</p>
                      <p className="text-xs text-slate-500">
                        {item.taskListName} ・ 最終 {item.lastWorkedDate}
                      </p>
                    </div>
                    <div className="shrink-0 text-right">
                      <p className="text-slate-300">
                        {formatMinutes(item.totalSeconds)}
                      </p>
                      <p className="text-xs text-slate-500">
                        {item.sessionCount}回
                        {item.completedCount > 0 && " ・ 完了"}
                      </p>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </>
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
