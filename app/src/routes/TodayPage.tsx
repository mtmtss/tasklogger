import { invoke } from "@tauri-apps/api/core";

export default function TodayPage() {
  return (
    <section>
      <div className="mb-4 flex items-center justify-between">
        <h1 className="text-xl font-bold">今日やるリスト</h1>
        <button
          onClick={() => void invoke("toggle_float_window")}
          className="rounded-md border border-slate-700 px-3 py-1.5 text-sm text-slate-300 hover:bg-slate-800"
        >
          フロート表示切替
        </button>
      </div>
      <p className="text-sm text-slate-400">
        M1 で実装予定: 今日期限のタスク一覧・開始/中断/完了・今日ダッシュボード
      </p>
    </section>
  );
}
