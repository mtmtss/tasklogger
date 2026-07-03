import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  connectGoogle,
  disconnectGoogle,
  getSettings,
  seedSampleData,
  setSetting,
} from "../lib/commands";
import { useSyncStatus } from "../lib/queries";

export default function SettingsPage() {
  const syncStatus = useSyncStatus();
  const queryClient = useQueryClient();
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [connecting, setConnecting] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [devMessage, setDevMessage] = useState<string | null>(null);
  const [closeToTray, setCloseToTray] = useState(true);

  useEffect(() => {
    getSettings()
      .then((settings) => {
        setClientId(settings["oauth_client_id"] ?? "");
        setClientSecret(settings["oauth_client_secret"] ?? "");
        setCloseToTray(settings["close_to_tray"] !== "false");
      })
      .catch(() => {});
  }, []);

  const handleCloseToTray = (checked: boolean) => {
    setCloseToTray(checked);
    void setSetting("close_to_tray", checked ? "true" : "false");
  };

  const refresh = () => void queryClient.invalidateQueries();

  const handleConnect = () => {
    setMessage(null);
    setConnecting(true);
    connectGoogle(clientId, clientSecret)
      .then(() => setMessage("Google と接続しました。バックグラウンドで同期を開始します。"))
      .catch((e) => setMessage(String(e)))
      .finally(() => {
        setConnecting(false);
        refresh();
      });
  };

  const handleDisconnect = () => {
    setMessage(null);
    disconnectGoogle()
      .then(() => setMessage("接続を解除しました。ローカルのみで動作します。"))
      .catch((e) => setMessage(String(e)))
      .finally(refresh);
  };

  const connected = syncStatus.data?.connected ?? false;

  return (
    <section className="max-w-2xl space-y-6">
      <h1 className="text-xl font-bold">設定</h1>

      <div className="rounded-lg border border-slate-800 bg-slate-900/60 p-4">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold text-slate-300">Google Tasks 連携</h2>
          <span
            className={`text-xs ${connected ? "text-emerald-400" : "text-slate-500"}`}
          >
            {connected ? "接続中" : "未接続"}
          </span>
        </div>

        {!connected && (
          <>
            <p className="mt-2 text-sm text-slate-500">
              Google Cloud Console で「デスクトップアプリ」用の OAuth クライアントを作成し、
              クライアント ID / シークレットを入力してください。
              手順は README の「Google 連携のセットアップ」を参照。
            </p>
            <div className="mt-3 space-y-2">
              <input
                value={clientId}
                onChange={(e) => setClientId(e.target.value)}
                placeholder="クライアント ID (…apps.googleusercontent.com)"
                className="w-full rounded-md border border-slate-700 bg-slate-950 px-3 py-2 text-sm placeholder:text-slate-600 focus:border-sky-600 focus:outline-none"
              />
              <input
                value={clientSecret}
                onChange={(e) => setClientSecret(e.target.value)}
                placeholder="クライアント シークレット"
                type="password"
                className="w-full rounded-md border border-slate-700 bg-slate-950 px-3 py-2 text-sm placeholder:text-slate-600 focus:border-sky-600 focus:outline-none"
              />
              <button
                onClick={handleConnect}
                disabled={connecting || !clientId.trim()}
                className="rounded-md bg-sky-600 px-4 py-2 text-sm text-white hover:bg-sky-500 disabled:opacity-50"
              >
                {connecting ? "ブラウザで承認待ち…" : "Google と接続"}
              </button>
            </div>
          </>
        )}

        {connected && (
          <div className="mt-3 space-y-2 text-sm text-slate-400">
            <p>
              最終同期:{" "}
              {syncStatus.data?.lastPullAt
                ? new Date(syncStatus.data.lastPullAt).toLocaleString("ja-JP")
                : "未実行"}
            </p>
            <p>未送信キュー: {syncStatus.data?.queueCount ?? 0} 件</p>
            <button
              onClick={handleDisconnect}
              className="rounded-md border border-rose-500/50 px-3 py-1.5 text-sm text-rose-300 hover:bg-rose-500/10"
            >
              接続を解除
            </button>
          </div>
        )}

        {message && <p className="mt-3 text-sm text-sky-300">{message}</p>}
      </div>

      <div className="rounded-lg border border-slate-800 bg-slate-900/60 p-4">
        <h2 className="text-sm font-semibold text-slate-300">動作</h2>
        <label className="mt-3 flex items-center gap-2 text-sm text-slate-300">
          <input
            type="checkbox"
            checked={closeToTray}
            onChange={(e) => handleCloseToTray(e.target.checked)}
            className="accent-sky-600"
          />
          閉じるボタンでトレイに常駐する (終了はトレイメニューから)
        </label>
      </div>

      <div className="rounded-lg border border-slate-800 bg-slate-900/60 p-4">
        <h2 className="text-sm font-semibold text-slate-300">開発用</h2>
        <p className="mt-1 text-sm text-slate-500">
          Google 未接続でも UI を試せるサンプルタスクを投入します。
        </p>
        <button
          onClick={() => {
            seedSampleData()
              .then(() => setDevMessage("サンプルタスクを投入しました。"))
              .catch((e) => setDevMessage(String(e)));
          }}
          className="mt-3 rounded-md border border-slate-700 px-3 py-1.5 text-sm text-slate-300 hover:bg-slate-800"
        >
          サンプルタスク投入
        </button>
        {devMessage && <p className="mt-2 text-sm text-sky-300">{devMessage}</p>}
      </div>
    </section>
  );
}
