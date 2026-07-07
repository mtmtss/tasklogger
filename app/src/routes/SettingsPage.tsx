import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  clearAiApiKey,
  connectGoogle,
  disconnectGoogle,
  exportCsv,
  getAiStatus,
  getSettings,
  importGasCsv,
  seedSampleData,
  setAiApiKey,
  setAutostart,
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
  const [idleMinutes, setIdleMinutes] = useState("5");
  const [autostart, setAutostartState] = useState(false);
  const [behaviorError, setBehaviorError] = useState<string | null>(null);

  useEffect(() => {
    getSettings()
      .then((settings) => {
        setClientId(settings["oauth_client_id"] ?? "");
        setClientSecret(settings["oauth_client_secret"] ?? "");
        setCloseToTray(settings["close_to_tray"] !== "false");
        setIdleMinutes(settings["idle_pause_minutes"] ?? "5");
        setAutostartState(settings["autostart"] === "true");
      })
      .catch(() => {});
  }, []);

  const handleAutostart = (checked: boolean) => {
    setBehaviorError(null);
    setAutostartState(checked);
    setAutostart(checked).catch((e) => {
      setAutostartState(!checked);
      setBehaviorError(String(e));
    });
  };

  const handleCloseToTray = (checked: boolean) => {
    setCloseToTray(checked);
    void setSetting("close_to_tray", checked ? "true" : "false");
  };

  const handleIdleMinutes = (value: string) => {
    setIdleMinutes(value);
    if (/^\d+$/.test(value.trim())) {
      void setSetting("idle_pause_minutes", value.trim());
    }
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
        <label className="mt-3 flex items-center gap-2 text-sm text-slate-300">
          無操作が
          <input
            type="number"
            min="0"
            value={idleMinutes}
            onChange={(e) => handleIdleMinutes(e.target.value)}
            className="w-16 rounded-md border border-slate-700 bg-slate-950 px-2 py-1 text-center text-sm focus:border-sky-600 focus:outline-none"
          />
          分続いたら実行中タスクを自動で中断する (0 で無効)
        </label>
        <p className="mt-1 text-xs text-slate-500">
          スリープ・画面ロック・スクリーンセーバ・離席をまとめて検知します。
          中断ログは最後に操作した時刻で締められます。
        </p>
        <label className="mt-3 flex items-center gap-2 text-sm text-slate-300">
          <input
            type="checkbox"
            checked={autostart}
            onChange={(e) => handleAutostart(e.target.checked)}
            className="accent-sky-600"
          />
          Windows サインイン時に自動起動する
        </label>
        {behaviorError && (
          <p className="mt-2 text-sm text-rose-300">{behaviorError}</p>
        )}
      </div>

      <SheetSyncSection connected={connected} />

      <AiSection />

      <DataSection />

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

/** AI 拡張「今日の作戦」設定 (docs/ai-extension-specification.md §4.3)。 */
function AiSection() {
  const aiStatus = useQuery({ queryKey: ["aiStatus"], queryFn: getAiStatus });
  const queryClient = useQueryClient();
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("gemini-2.5-flash");
  const [userContext, setUserContext] = useState("");
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    getSettings()
      .then((settings) => {
        if (settings["ai_model"]) setModel(settings["ai_model"]);
        setUserContext(settings["ai_user_context"] ?? "");
      })
      .catch(() => {});
  }, []);

  const configured = aiStatus.data?.configured ?? false;
  const refresh = () =>
    void queryClient.invalidateQueries({ queryKey: ["aiStatus"] });

  const handleSaveKey = () => {
    setMessage(null);
    setSaving(true);
    setAiApiKey(apiKey)
      .then(() => {
        setMessage("接続を確認して保存しました。今日ページに「今日の作戦」カードが表示されます。");
        setApiKey("");
      })
      .catch((e) => setMessage(String(e)))
      .finally(() => {
        setSaving(false);
        refresh();
      });
  };

  const handleClearKey = () => {
    clearAiApiKey()
      .then(() => setMessage("API キーを削除しました。AI 機能は無効になります。"))
      .catch((e) => setMessage(String(e)))
      .finally(refresh);
  };

  const handleModel = (value: string) => {
    setModel(value);
    void setSetting("ai_model", value);
  };

  const handleUserContext = (value: string) => {
    setUserContext(value);
    void setSetting("ai_user_context", value);
  };

  return (
    <div className="rounded-lg border border-slate-800 bg-slate-900/60 p-4">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-slate-300">
          AI「今日の作戦」(Gemini API)
        </h2>
        <span
          className={`text-xs ${configured ? "text-emerald-400" : "text-slate-500"}`}
        >
          {configured ? "有効" : "未設定"}
        </span>
      </div>
      <p className="mt-1 text-xs text-slate-500">
        キーは Google AI Studio (aistudio.google.com) で無料発行できます。
        有効にすると、生成時にタスクのタイトル・メモ・作業実績の要約が Google の
        Gemini API に送信されます (キーの設定 = 同意)。
        無料枠ではデータがサービス改善に利用される場合があります —
        機微な内容を扱う場合は課金を有効にしたキーを推奨。キーは Windows
        資格情報マネージャーにのみ保存されます。
      </p>

      {!configured ? (
        <div className="mt-3 flex items-center gap-2">
          <input
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder="Gemini API キー (AIza…)"
            type="password"
            className="min-w-0 flex-1 rounded-md border border-slate-700 bg-slate-950 px-3 py-2 text-sm placeholder:text-slate-600 focus:border-violet-500 focus:outline-none"
          />
          <button
            onClick={handleSaveKey}
            disabled={saving || !apiKey.trim()}
            className="shrink-0 rounded-md bg-violet-600 px-3 py-2 text-sm text-white hover:bg-violet-500 disabled:opacity-50"
          >
            {saving ? "接続確認中…" : "接続して保存"}
          </button>
        </div>
      ) : (
        <button
          onClick={handleClearKey}
          className="mt-3 rounded-md border border-rose-500/50 px-3 py-1.5 text-sm text-rose-300 hover:bg-rose-500/10"
        >
          API キーを削除 (AI 機能を無効化)
        </button>
      )}

      <div className="mt-4 space-y-3 border-t border-slate-800 pt-3">
        <label className="flex items-center gap-2 text-sm text-slate-300">
          モデル
          <select
            value={model}
            onChange={(e) => handleModel(e.target.value)}
            className="rounded-md border border-slate-700 bg-slate-950 px-2 py-1 text-sm"
          >
            <option value="gemini-2.5-flash">gemini-2.5-flash (推奨・無料枠あり)</option>
            <option value="gemini-2.5-pro">gemini-2.5-pro (品質優先)</option>
            <option value="gemini-2.5-flash-lite">gemini-2.5-flash-lite (最速・最安)</option>
          </select>
        </label>
        <div>
          <label className="text-sm text-slate-300">
            プロファイル (作戦生成の前提として毎回使われます)
          </label>
          <textarea
            value={userContext}
            onChange={(e) => handleUserContext(e.target.value)}
            placeholder="例: 情報セキュリティの研究者。午前は集中作業向き。発表準備を研究実装より優先しがちなので注意。"
            rows={4}
            className="mt-1 w-full rounded-md border border-slate-700 bg-slate-950 px-3 py-2 text-sm placeholder:text-slate-600 focus:border-violet-500 focus:outline-none"
          />
        </div>
      </div>

      {message && <p className="mt-3 text-sm text-sky-300">{message}</p>}
    </div>
  );
}

/** 作業ログの Google Sheets 同期 (spec §6.6)。複数デバイスのログを合流させる。 */
function SheetSyncSection({ connected }: { connected: boolean }) {
  const [enabled, setEnabled] = useState(false);
  const [spreadsheetId, setSpreadsheetId] = useState("");
  const [lastSyncAt, setLastSyncAt] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    getSettings()
      .then((settings) => {
        setEnabled(settings["sheet_sync_enabled"] === "true");
        setSpreadsheetId(settings["log_spreadsheet_id"] ?? "");
        setLastSyncAt(settings["last_sheet_sync_at"] ?? null);
      })
      .catch(() => {});
  }, []);

  const handleToggle = (checked: boolean) => {
    setEnabled(checked);
    setMessage(
      checked
        ? "次回の同期サイクルから有効になります。初回は Google 接続のやり直し (権限追加) が必要な場合があります。"
        : "無効にしました。",
    );
    void setSetting("sheet_sync_enabled", checked ? "true" : "false");
  };

  const handleSpreadsheetId = (value: string) => {
    setSpreadsheetId(value);
    void setSetting("log_spreadsheet_id", value.trim());
  };

  return (
    <div className="rounded-lg border border-slate-800 bg-slate-900/60 p-4">
      <h2 className="text-sm font-semibold text-slate-300">
        作業ログの Sheets 同期 (複数デバイス統合)
      </h2>
      <p className="mt-1 text-sm text-slate-500">
        作業ログを Google スプレッドシートと双方向同期し、複数の PC
        の記録を統合します。同期はバックグラウンドで行われ、操作の速度には影響しません。
      </p>

      <label className="mt-3 flex items-center gap-2 text-sm text-slate-300">
        <input
          type="checkbox"
          checked={enabled}
          onChange={(e) => handleToggle(e.target.checked)}
          disabled={!connected}
          className="accent-sky-600"
        />
        Sheets 同期を有効にする
        {!connected && (
          <span className="text-xs text-slate-500">(先に Google と接続してください)</span>
        )}
      </label>

      {enabled && (
        <div className="mt-3 space-y-2">
          <input
            value={spreadsheetId}
            onChange={(e) => handleSpreadsheetId(e.target.value)}
            placeholder="スプレッドシート ID (空欄なら自動作成。GAS 版の ID も指定可)"
            className="w-full rounded-md border border-slate-700 bg-slate-950 px-3 py-2 text-sm placeholder:text-slate-600 focus:border-sky-600 focus:outline-none"
          />
          <p className="text-xs text-slate-500">
            {spreadsheetId
              ? `同期先: ${spreadsheetId}`
              : "空欄のまま同期すると「TaskLogger Logs」を自動作成し、ここに ID が入ります。"}
            {lastSyncAt &&
              ` ・ 最終同期: ${new Date(lastSyncAt).toLocaleString("ja-JP")}`}
          </p>
          <p className="text-xs text-slate-500">
            GAS 版の Spreadsheet ID を指定すると、過去ログとそのまま合流できます
            (旧データも自動で取り込まれます)。
          </p>
        </div>
      )}

      {message && <p className="mt-2 text-sm text-sky-300">{message}</p>}
    </div>
  );
}

/** 作業ログの CSV エクスポート / GAS 旧データインポート (spec §5.6)。 */
function DataSection() {
  const [exportStart, setExportStart] = useState("2020-01-01");
  const [exportEnd, setExportEnd] = useState(() =>
    new Date().toISOString().slice(0, 10),
  );
  const [message, setMessage] = useState<string | null>(null);
  const queryClient = useQueryClient();

  const handleExport = () => {
    setMessage(null);
    exportCsv(exportStart, exportEnd)
      .then((path) =>
        setMessage(path ? `保存しました: ${path}` : "キャンセルしました。"),
      )
      .catch((e) => setMessage(String(e)));
  };

  const handleImport = () => {
    setMessage(null);
    importGasCsv()
      .then((result) => {
        if (!result) {
          setMessage("キャンセルしました。");
          return;
        }
        setMessage(
          `取り込み ${result.imported} 件、スキップ (重複等) ${result.skipped} 件`,
        );
        void queryClient.invalidateQueries();
      })
      .catch((e) => setMessage(String(e)));
  };

  return (
    <div className="rounded-lg border border-slate-800 bg-slate-900/60 p-4">
      <h2 className="text-sm font-semibold text-slate-300">作業ログのデータ</h2>

      <div className="mt-3 flex flex-wrap items-center gap-2 text-sm">
        <input
          type="date"
          value={exportStart}
          onChange={(e) => setExportStart(e.target.value)}
          className="rounded-md border border-slate-700 bg-slate-950 px-2 py-1.5 text-sm [color-scheme:dark]"
        />
        <span className="text-slate-500">〜</span>
        <input
          type="date"
          value={exportEnd}
          onChange={(e) => setExportEnd(e.target.value)}
          className="rounded-md border border-slate-700 bg-slate-950 px-2 py-1.5 text-sm [color-scheme:dark]"
        />
        <button
          onClick={handleExport}
          className="rounded-md border border-slate-700 px-3 py-1.5 text-sm text-slate-300 hover:bg-slate-800"
        >
          CSV エクスポート
        </button>
      </div>

      <div className="mt-4 border-t border-slate-800 pt-3">
        <p className="text-sm text-slate-500">
          GAS 版の WorkLogs シートを CSV 保存したファイルを取り込みます (logId
          重複は自動スキップ)。
        </p>
        <button
          onClick={handleImport}
          className="mt-2 rounded-md border border-slate-700 px-3 py-1.5 text-sm text-slate-300 hover:bg-slate-800"
        >
          GAS データをインポート
        </button>
      </div>

      {message && <p className="mt-3 text-sm text-sky-300">{message}</p>}
    </div>
  );
}
