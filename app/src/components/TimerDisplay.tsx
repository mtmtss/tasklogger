import { useEffect, useState } from "react";
import { elapsedSince, formatElapsed } from "../lib/format";

/** startAt から毎秒ローカル計算する経過タイマー (IPC なし, spec §8.3)。 */
export default function TimerDisplay({
  startAt,
  className,
}: {
  startAt: string;
  className?: string;
}) {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);

  return (
    <span className={`tabular-nums ${className ?? ""}`}>
      {formatElapsed(elapsedSince(startAt, now))}
    </span>
  );
}
