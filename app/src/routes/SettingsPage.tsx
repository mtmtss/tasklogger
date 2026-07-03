import { useState } from "react";
import { seedSampleData } from "../lib/commands";

export default function SettingsPage() {
  const [message, setMessage] = useState<string | null>(null);

  return (
    <section className="space-y-6">
      <h1 className="text-xl font-bold">設定</h1>

      <div className="rounded-lg border border-slate-800 bg-slate-900/60 p-4">
        <h2 className="text-sm font-semibold text-slate-300">Google 連携</h2>
        <p className="mt-1 text-sm text-slate-500">
          M2 で実装予定: OAuth 接続・同期状態の表示
        </p>
      </div>

      <div className="rounded-lg border border-slate-800 bg-slate-900/60 p-4">
        <h2 className="text-sm font-semibold text-slate-300">開発用</h2>
        <p className="mt-1 text-sm text-slate-500">
          Google 未接続でも UI を試せるサンプルタスクを投入します。
        </p>
        <button
          onClick={() => {
            seedSampleData()
              .then(() => setMessage("サンプルタスクを投入しました。"))
              .catch((e) => setMessage(String(e)));
          }}
          className="mt-3 rounded-md border border-slate-700 px-3 py-1.5 text-sm text-slate-300 hover:bg-slate-800"
        >
          サンプルタスク投入
        </button>
        {message && <p className="mt-2 text-sm text-sky-300">{message}</p>}
      </div>
    </section>
  );
}
