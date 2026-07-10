import { useContext, useRef, useState } from "react";
import {
  DateField,
  DateInput,
  DateSegment,
  I18nProvider,
  TimeField,
  TimeFieldStateContext,
} from "react-aria-components";
import { CalendarDate, parseDate, Time } from "@internationalized/date";

interface Props {
  taskTitle: string;
  currentDue: string | null;
  onConfirm: (due: string | null) => void;
  onCancel: () => void;
}

function pad2(n: number): string {
  return String(n).padStart(2, "0");
}

/** currentDue ("yyyy-MM-ddT..Z") の日付部分だけを取り出す。 */
function initialDate(currentDue: string | null): CalendarDate {
  if (!currentDue) {
    const now = new Date();
    return new CalendarDate(now.getFullYear(), now.getMonth() + 1, now.getDate());
  }
  return parseDate(currentDue.slice(0, 10));
}

/** 期限切れタスクの期限変更用ダイアログ。日付/時刻それぞれトグルでON/OFFする。
 * ネイティブの date/time input は macOS の WKWebView では OS のリージョン設定に
 * 固定され `lang` 属性でも変更できず、wojtekmaj/react-date-picker系はセグメント
 * 単位の状態管理が甘くキー入力の取りこぼしがあったため、React Aria Components の
 * DateField/TimeField(各セグメントが独立してフォーカス・編集できる、キーボード
 * 操作の実装が堅牢なライブラリ)に置き換えた。 */
export default function DueDatePrompt({
  taskTitle,
  currentDue,
  onConfirm,
  onCancel,
}: Props) {
  const [dateEnabled, setDateEnabled] = useState(true);
  const [dateValue, setDateValue] = useState<CalendarDate>(initialDate(currentDue));
  const [timeEnabled, setTimeEnabled] = useState(false);
  const [timeValue, setTimeValue] = useState<Time>(new Time(0, 0));
  const [focusedTimeType, setFocusedTimeType] = useState<"hour" | "minute">("hour");
  const timeSegmentRefs = useRef<Partial<Record<"hour" | "minute", HTMLElement | null>>>({});

  const handleSave = () => {
    if (!dateEnabled) {
      onConfirm(null);
      return;
    }
    const y = dateValue.year;
    const m = pad2(dateValue.month);
    const d = pad2(dateValue.day);
    const hh = timeEnabled ? pad2(timeValue.hour) : "00";
    const mm = timeEnabled ? pad2(timeValue.minute) : "00";
    onConfirm(`${y}-${m}-${d}T${hh}:${mm}:00.000Z`);
  };

  return (
    <I18nProvider locale="ja-JP">
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
        <div className="w-full max-w-md rounded-xl border border-slate-700 bg-slate-900 p-6 shadow-xl">
          <h2 className="text-lg font-semibold">期限を変更</h2>
          <p className="mt-2 text-sm text-slate-400">
            <span className="font-medium text-slate-200">「{taskTitle}」</span>
            の元の期限: {currentDue ? currentDue.slice(0, 10) : "なし"}
          </p>

          <div className="mt-4 rounded-lg border border-slate-800 bg-slate-950">
            <div className="flex items-center justify-between gap-3 px-4 py-3">
              <div>
                <p className="text-sm text-slate-200">日付</p>
                {dateEnabled ? (
                  <DateField
                    value={dateValue}
                    onChange={(v) => v && setDateValue(v)}
                    shouldForceLeadingZeros
                    className="mt-1"
                  >
                    <DateInput className="due-date-input">
                      {(segment) => (
                        <DateSegment segment={segment} className="due-date-segment" />
                      )}
                    </DateInput>
                  </DateField>
                ) : (
                  <p className="text-xs text-slate-500">未設定</p>
                )}
              </div>
              <Toggle checked={dateEnabled} onChange={setDateEnabled} />
            </div>
            <div className="border-t border-slate-800" />
            <div className="flex items-center justify-between gap-3 px-4 py-3">
              <div>
                <p className={`text-sm ${dateEnabled ? "text-slate-200" : "text-slate-600"}`}>
                  時刻
                </p>
                {timeEnabled && dateEnabled ? (
                  <TimeField
                    value={timeValue}
                    onChange={(v) => v && setTimeValue(v)}
                    hourCycle={24}
                    granularity="minute"
                    shouldForceLeadingZeros
                    onFocus={(e) => {
                      const type = (e.target as HTMLElement).dataset.type;
                      if (type === "hour" || type === "minute") setFocusedTimeType(type);
                    }}
                    className="mt-1 flex items-center gap-1"
                  >
                    <DateInput className="due-date-input">
                      {(segment) => (
                        <DateSegment
                          segment={segment}
                          ref={(el) => {
                            if (segment.type === "hour" || segment.type === "minute") {
                              timeSegmentRefs.current[segment.type] = el;
                            }
                          }}
                          className="due-date-segment"
                        />
                      )}
                    </DateInput>
                    <TimeStepper
                      focusedType={focusedTimeType}
                      focusSegment={(type) => timeSegmentRefs.current[type]?.focus()}
                    />
                  </TimeField>
                ) : (
                  <p className="text-xs text-slate-500">未設定</p>
                )}
              </div>
              <Toggle
                checked={timeEnabled && dateEnabled}
                onChange={setTimeEnabled}
                disabled={!dateEnabled}
              />
            </div>
          </div>

          <div className="mt-5 flex justify-end gap-2">
            <button
              onClick={onCancel}
              className="rounded-md border border-slate-700 px-4 py-2 text-sm text-slate-300 hover:bg-slate-800"
            >
              キャンセル
            </button>
            <button
              onClick={handleSave}
              className="rounded-md bg-sky-600 px-4 py-2 text-sm text-white hover:bg-sky-500"
            >
              保存
            </button>
          </div>
        </div>
      </div>
    </I18nProvider>
  );
}

/** 直近でフォーカスされた時刻セグメント(時/分)を増減する上下ボタン。
 * TimeField 内部の state (increment/decrement) を TimeFieldStateContext 経由で操作する。 */
function TimeStepper({
  focusedType,
  focusSegment,
}: {
  focusedType: "hour" | "minute";
  focusSegment: (type: "hour" | "minute") => void;
}) {
  const state = useContext(TimeFieldStateContext);
  if (!state) return null;

  const step = (delta: 1 | -1) => {
    if (delta === 1) state.increment(focusedType);
    else state.decrement(focusedType);
    // フォーカス済みセグメントがなくても、操作対象を青ハイライトで示す
    focusSegment(focusedType);
  };

  return (
    <div className="flex flex-col overflow-hidden rounded-md border border-slate-700">
      <button
        type="button"
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => step(1)}
        className="px-1.5 py-0.5 text-[9px] leading-none text-slate-300 hover:bg-slate-700"
      >
        ▲
      </button>
      <div className="h-px bg-slate-700" />
      <button
        type="button"
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => step(-1)}
        className="px-1.5 py-0.5 text-[9px] leading-none text-slate-300 hover:bg-slate-700"
      >
        ▼
      </button>
    </div>
  );
}

function Toggle({
  checked,
  onChange,
  disabled,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative h-6 w-11 shrink-0 rounded-full transition-colors ${
        checked ? "bg-sky-600" : "bg-slate-700"
      } ${disabled ? "cursor-not-allowed opacity-40" : ""}`}
    >
      <span
        className={`absolute left-0.5 top-0.5 h-5 w-5 rounded-full bg-white transition-transform ${
          checked ? "translate-x-5" : "translate-x-0"
        }`}
      />
    </button>
  );
}
