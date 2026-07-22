/** 秒を "H:MM:SS" (1時間未満は "M:SS") に整形する。 */
export function formatElapsed(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds));
  const hours = Math.floor(s / 3600);
  const minutes = Math.floor((s % 3600) / 60);
  const seconds = s % 60;
  const mm = String(minutes).padStart(2, "0");
  const ss = String(seconds).padStart(2, "0");
  return hours > 0 ? `${hours}:${mm}:${ss}` : `${minutes}:${ss}`;
}

/** 秒を "◯時間◯分" / "◯分" に整形する (集計表示用)。 */
export function formatMinutes(totalSeconds: number): string {
  const minutes = Math.ceil(Math.max(0, totalSeconds) / 60);
  if (minutes < 60) return `${minutes}分`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest === 0 ? `${hours}時間` : `${hours}時間${rest}分`;
}

/** ISO 文字列からの経過秒。 */
export function elapsedSince(startAtIso: string, nowMs: number): number {
  return Math.max(0, Math.floor((nowMs - new Date(startAtIso).getTime()) / 1000));
}

/** "2026-07-05" → "7月5日(土)" */
export function formatJapaneseDate(dateText: string): string {
  const [y, m, d] = dateText.split("-").map(Number);
  if (!y || !m || !d) return dateText;
  const youbi = ["日", "月", "火", "水", "木", "金", "土"][
    new Date(y, m - 1, d).getDay()
  ];
  return `${m}月${d}日(${youbi})`;
}

/** "2026-07-05" → "7/5(土)" (狭いラベル向け) */
export function formatShortDate(dateText: string): string {
  const [y, m, d] = dateText.split("-").map(Number);
  if (!y || !m || !d) return dateText;
  const youbi = ["日", "月", "火", "水", "木", "金", "土"][
    new Date(y, m - 1, d).getDay()
  ];
  return `${m}/${d}(${youbi})`;
}
