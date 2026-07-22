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
import { getArchiveAnalytics, getRangeLogs, type WorkLogEntry } from "../lib/commands";
import { formatMinutes, formatShortDate } from "../lib/format";

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

function addDays(date: string, days: number): string {
  const [y, m, d] = date.split("-").map(Number);
  const next = new Date(y, m - 1, d);
  next.setDate(next.getDate() + days);
  return dateText(next);
}

/** 開始日から終了日までの日数 (両端含む)。 */
function daysBetween(start: string, end: string): number {
  const [ys, ms, ds] = start.split("-").map(Number);
  const [ye, me, de] = end.split("-").map(Number);
  const a = Date.UTC(ys, ms - 1, ds);
  const b = Date.UTC(ye, me - 1, de);
  return Math.round((b - a) / 86_400_000) + 1;
}

function buildDateRange(start: string, end: string): string[] {
  const dates: string[] = [];
  let cursor = start;
  while (cursor <= end && dates.length < 400) {
    dates.push(cursor);
    cursor = addDays(cursor, 1);
  }
  return dates;
}

function formatTime(iso: string): string {
  return new Date(iso).toLocaleTimeString("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}

/** カテゴリカル配色 (dark surface 用に検証済みの8色, 順序固定)。9件目以降は灰色に畳む。 */
const TASK_LIST_COLORS = [
  "#3987e5",
  "#d95926",
  "#199e70",
  "#c98500",
  "#d55181",
  "#008300",
  "#9085e9",
  "#e66767",
];
const OTHER_COLOR = "#64748b";

const HOUR_TICKS = [0, 3, 6, 9, 12, 15, 18, 21, 24];

/** これを超える日数のレンジではタイムラインが行数過多になるため日別推移グラフにフォールバックする。 */
const TIMELINE_MAX_DAYS = 14;

const PRESETS = [
  { label: "7日", days: 6 },
  { label: "30日", days: 29 },
  { label: "90日", days: 89 },
];

export default function ArchivePage() {
  const [startDate, setStartDate] = useState(() => daysAgo(6));
  const [endDate, setEndDate] = useState(() => dateText(new Date()));
  const [taskLimit, setTaskLimit] = useState(20);
  const [chartView, setChartView] = useState<"daily" | "timeline">("daily");

  const dayCount = daysBetween(startDate, endDate);
  const timelineAvailable = dayCount <= TIMELINE_MAX_DAYS;

  const { data, error } = useQuery({
    queryKey: ["archive", startDate, endDate],
    queryFn: () => getArchiveAnalytics(startDate, endDate),
  });

  const { data: logs } = useQuery({
    queryKey: ["archive-logs", startDate, endDate],
    queryFn: () => getRangeLogs(startDate, endDate),
    enabled: chartView === "timeline" && timelineAvailable,
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
            <div className="mb-3 flex items-center justify-between">
              <h2 className="text-sm font-semibold text-slate-400">
                {chartView === "timeline" ? "時間帯別タイムライン" : "日別推移"}
              </h2>
              <div className="flex items-center gap-1 rounded-md border border-slate-700 bg-slate-950/40 p-1 text-xs">
                <button
                  onClick={() => setChartView("daily")}
                  className={`rounded px-2.5 py-1 transition-colors ${
                    chartView === "daily"
                      ? "bg-sky-600 text-white"
                      : "text-slate-400 hover:text-slate-200"
                  }`}
                >
                  日別推移
                </button>
                <button
                  onClick={() => setChartView("timeline")}
                  disabled={!timelineAvailable}
                  title={timelineAvailable ? undefined : `タイムラインは${TIMELINE_MAX_DAYS}日以内の期間で利用できます`}
                  className={`rounded px-2.5 py-1 transition-colors ${
                    chartView === "timeline"
                      ? "bg-sky-600 text-white"
                      : timelineAvailable
                        ? "text-slate-400 hover:text-slate-200"
                        : "text-slate-700"
                  }`}
                >
                  タイムライン
                </button>
              </div>
            </div>

            {chartView === "timeline" && timelineAvailable ? (
              logs && <RangeTimeline dates={buildDateRange(startDate, endDate)} entries={logs} />
            ) : (
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
            )}
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

/** タスクリストIDに固定の配色を割り当てる (登場順、8件超過分は灰色に畳む)。 */
function useTaskListColors(entries: WorkLogEntry[]) {
  const order: string[] = [];
  for (const item of entries) {
    if (!order.includes(item.taskListId)) order.push(item.taskListId);
  }
  const colorFor = (taskListId: string) => {
    const idx = order.indexOf(taskListId);
    return idx >= 0 && idx < TASK_LIST_COLORS.length ? TASK_LIST_COLORS[idx] : OTHER_COLOR;
  };
  const legend = order.map((id) => ({
    id,
    name: entries.find((e) => e.taskListId === id)?.taskListName ?? id,
    color: colorFor(id),
  }));
  return { colorFor, legend };
}

function RangeTimeline({ dates, entries }: { dates: string[]; entries: WorkLogEntry[] }) {
  const { colorFor, legend } = useTaskListColors(entries);
  const today = dateText(new Date());

  return (
    <div>
      <div
        className="grid items-center gap-y-1.5"
        style={{ gridTemplateColumns: "3.75rem 1fr" }}
      >
        <div />
        <div className="relative h-4 text-[10px] text-slate-500">
          {HOUR_TICKS.map((h) => (
            <span
              key={h}
              className="absolute -translate-x-1/2 first:translate-x-0 last:-translate-x-full"
              style={{ left: `${(h / 24) * 100}%` }}
            >
              {h}:00
            </span>
          ))}
        </div>
        {dates.map((date) => (
          <TimelineRow
            key={date}
            date={date}
            entries={entries}
            colorFor={colorFor}
            isToday={date === today}
          />
        ))}
      </div>

      {legend.length > 1 && (
        <div className="mt-4 flex flex-wrap gap-x-4 gap-y-1.5 border-t border-slate-800 pt-3">
          {legend.map((entry) => (
            <div key={entry.id} className="flex items-center gap-1.5 text-xs text-slate-400">
              <span
                className="h-2.5 w-2.5 shrink-0 rounded-full"
                style={{ backgroundColor: entry.color }}
              />
              <span className="truncate">{entry.name}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function TimelineRow({
  date,
  entries,
  colorFor,
  isToday,
}: {
  date: string;
  entries: WorkLogEntry[];
  colorFor: (taskListId: string) => string;
  isToday: boolean;
}) {
  const [y, m, d] = date.split("-").map(Number);
  const dayStart = new Date(y, m - 1, d, 0, 0, 0).getTime();
  const dayMs = 24 * 60 * 60 * 1000;
  const dayEnd = dayStart + dayMs;

  const visible = entries.filter((item) => {
    const start = new Date(item.startTime).getTime();
    const end = new Date(item.endTime).getTime();
    if (item.durationSeconds === 0 && item.endReason === "direct_complete") {
      return start >= dayStart && start < dayEnd;
    }
    return start < dayEnd && end > dayStart;
  });

  return (
    <>
      <div
        className={`pr-2 text-right text-[11px] ${
          isToday ? "font-semibold text-sky-400" : "text-slate-500"
        }`}
      >
        {formatShortDate(date)}
      </div>
      <div className="relative h-7 rounded bg-slate-950/40">
        {HOUR_TICKS.map((h) => (
          <div
            key={h}
            className="absolute top-0 h-full w-px bg-slate-800/60"
            style={{ left: `${(h / 24) * 100}%` }}
          />
        ))}
        {visible.map((item) => (
          <TimelineMark
            key={item.logId}
            item={item}
            dayStart={dayStart}
            dayEnd={dayEnd}
            color={colorFor(item.taskListId)}
          />
        ))}
      </div>
    </>
  );
}

function TimelineMark({
  item,
  dayStart,
  dayEnd,
  color,
}: {
  item: WorkLogEntry;
  dayStart: number;
  dayEnd: number;
  color: string;
}) {
  const dayMs = dayEnd - dayStart;
  const start = new Date(item.startTime).getTime();
  const end = new Date(item.endTime).getTime();
  const isInstant = item.durationSeconds === 0 && item.endReason === "direct_complete";
  const timeLabel = isInstant
    ? formatTime(item.startTime)
    : `${formatTime(item.startTime)}–${formatTime(item.endTime)}`;
  const detailLabel = isInstant ? "即時完了" : formatMinutes(item.durationSeconds);

  if (isInstant) {
    const leftPct = ((start - dayStart) / dayMs) * 100;
    return (
      <div
        tabIndex={0}
        className="group absolute top-1/2 -translate-x-1/2 -translate-y-1/2 focus:outline-none"
        style={{ left: `${leftPct}%` }}
      >
        <span
          className="block h-2 w-2 rounded-full ring-2 ring-slate-950/50"
          style={{ backgroundColor: color }}
        />
        <TimelineTooltip
          title={item.taskTitle}
          taskListName={item.taskListName}
          timeLabel={timeLabel}
          detailLabel={detailLabel}
        />
      </div>
    );
  }

  // 日をまたぐセッションはこの行の 0:00〜24:00 の範囲に切り詰めて描画し、
  // 続きは次の日の行の 0:00 側から表示する (右端にはみ出させない)。
  const clippedStart = Math.max(start, dayStart);
  const clippedEnd = Math.min(end, dayEnd);
  const continuesBefore = start < dayStart;
  const continuesAfter = end > dayEnd;
  const leftPct = ((clippedStart - dayStart) / dayMs) * 100;
  const widthPct = Math.max(((clippedEnd - clippedStart) / dayMs) * 100, 0.4);

  return (
    <div
      tabIndex={0}
      className="group absolute top-1/2 h-4 -translate-y-1/2 focus:outline-none"
      style={{
        left: `${leftPct}%`,
        width: `${widthPct}%`,
        backgroundColor: color,
        boxShadow: "0 0 0 2px rgba(2, 6, 23, 0.6)",
        borderTopLeftRadius: continuesBefore ? 0 : 4,
        borderBottomLeftRadius: continuesBefore ? 0 : 4,
        borderTopRightRadius: continuesAfter ? 0 : 4,
        borderBottomRightRadius: continuesAfter ? 0 : 4,
      }}
    >
      <TimelineTooltip
        title={item.taskTitle}
        taskListName={item.taskListName}
        timeLabel={timeLabel}
        detailLabel={detailLabel}
      />
    </div>
  );
}

function TimelineTooltip({
  title,
  taskListName,
  timeLabel,
  detailLabel,
}: {
  title: string;
  taskListName: string;
  timeLabel: string;
  detailLabel: string;
}) {
  return (
    <div className="pointer-events-none absolute bottom-full left-1/2 z-10 mb-2 hidden -translate-x-1/2 whitespace-nowrap rounded-md border border-slate-700 bg-slate-900 px-2.5 py-1.5 text-xs shadow-lg group-hover:block group-focus:block">
      <p className="font-semibold text-slate-100">
        {timeLabel} ・ {detailLabel}
      </p>
      <p className="text-slate-400">
        {title} ({taskListName})
      </p>
    </div>
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
