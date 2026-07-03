import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { syncNow } from "../lib/commands";
import { useSyncStatus } from "../lib/queries";

/** 同期状態バッジ + 手動同期 (spec §6.4)。 */
export default function SyncBadge({
  onError,
}: {
  onError?: (message: string) => void;
}) {
  const status = useSyncStatus();
  const queryClient = useQueryClient();
  const [syncing, setSyncing] = useState(false);

  const data = status.data;
  if (!data) return null;

  const handleSync = () => {
    setSyncing(true);
    syncNow()
      .catch((e) => onError?.(String(e)))
      .finally(() => {
        setSyncing(false);
        void queryClient.invalidateQueries();
      });
  };

  return (
    <div className="flex items-center gap-2 text-xs">
      {data.connected ? (
        <>
          <span className="flex items-center gap-1.5 text-slate-400">
            <span className="inline-block h-1.5 w-1.5 rounded-full bg-emerald-400" />
            Google 接続中
          </span>
          {data.queueCount > 0 && (
            <span className="rounded-full bg-amber-500/15 px-2 py-0.5 text-amber-300">
              未送信 {data.queueCount}
            </span>
          )}
          <button
            onClick={handleSync}
            disabled={syncing}
            className="rounded-md border border-slate-700 px-2 py-1 text-slate-300 hover:bg-slate-800 disabled:opacity-50"
          >
            {syncing ? "同期中…" : "同期"}
          </button>
        </>
      ) : (
        <span className="flex items-center gap-1.5 text-slate-500">
          <span className="inline-block h-1.5 w-1.5 rounded-full bg-slate-600" />
          オフライン (ローカルのみ)
        </span>
      )}
    </div>
  );
}
