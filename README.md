# TaskLogger

日々のタスク実行時間を記録・可視化するタスク管理アプリです。

現在、GAS(Google Apps Script)版から **Tauri 製デスクトップアプリ版へ移行中**です。

## リポジトリ構成

```text
tasklogger/
├── docs/
│   └── specification.md   # Tauri 版仕様書 (最新・正)
├── app/                   # Tauri 版アプリ本体 (実装予定)
├── apps_script/           # 旧 GAS 版 (凍結・参照用)
└── README.md
```

| ディレクトリ | 状態 | 内容 |
| --- | --- | --- |
| [docs/specification.md](docs/specification.md) | **最新** | Tauri 版の仕様書。設計思想・データモデル・状態遷移・同期設計・実装マイルストーンを定義 |
| `app/` | scaffold 済み(M1 実装中) | Tauri 2.x + React + TypeScript のデスクトップアプリ |
| `apps_script/` | 凍結 | 旧 GAS 版のソースと仕様([apps_script/specification.txt](apps_script/specification.txt))。新規セットアップは非推奨 |

## Tauri 版の概要

GAS 版の設計思想(Google Tasks をタスクの源泉とし、開始→中断/完了の追記型ログで作業時間を記録する)を保ったまま、以下をアップデートします。

- **ローカルファースト化**: 全操作はローカル SQLite に即時反映(GAS 版の速度問題を解消)。Google Tasks への反映はバックグラウンド同期
- **中断中タスクの直接完了**: 再開せずにそのまま完了できる
- **「今すぐやる」ボタン**: 候補タスクを期限today化と同時に実行中にする(旧「今日やる」の置き換え)
- **PC スリープ検知**: スリープ時に自動中断、復帰時に再開ダイアログ
- **常時最前面フロートウィンドウ**: 現在タスクと経過タイマーを常時表示、中断/完了操作も可能
- **UI 全面刷新**: React + TypeScript + Tailwind CSS

詳細はすべて [docs/specification.md](docs/specification.md) を参照してください。

### 実装状況(マイルストーン)

- [x] M0: 仕様書作成
- [x] M0: `app/` scaffold
- [x] M1: ローカルコア(SQLite・状態機械・今日リスト・開始/中断/完了)
- [x] M2: Google Tasks 連携(OAuth・pull/push 同期・今すぐやる)
- [x] M3: スリープ検知・起動時回復・フロートウィンドウ・トレイ
- [x] M4: アーカイブ分析・CSV エクスポート・GAS データインポート
- [x] M5: 磨き込み・インストーラ

すべてのマイルストーンが完了しています。インストーラは `app/` で `npm run tauri build` を実行すると `app/src-tauri/target/release/bundle/` に生成されます (NSIS `*-setup.exe` / MSI)。

---

## 旧 GAS 版について(凍結)

`apps_script/` は Google Tasks + Google Apps Script + Spreadsheet で動作する旧版です。既存環境の運用継続・データ移行のために残していますが、新規機能追加は行いません。

- 仕様: [apps_script/specification.txt](apps_script/specification.txt)
- 蓄積済みの作業ログ(WorkLogs シート)は、Tauri 版の「GAS データインポート」機能(M4)で CSV 経由で引き継げます(スキーマ互換は仕様書 §3.2 参照)

<details>
<summary>旧版のセットアップ手順(参考)</summary>

### 前提条件

- Google アカウント / Node.js / `clasp`

```bash
npm install -g @google/clasp
clasp login
```

### 手順

1. リポジトリを取得し `apps_script/` へ移動
2. `clasp create --type standalone --title "TaskLogger_GAS"`
3. `clasp push`
4. Apps Script エディタで `Tasks API`(高度なサービス)を追加
5. `initializeTaskLoggerProject` を 1 回実行(Spreadsheet 作成・シート初期化)
6. 必要なら `settings` シート / Script Properties に設定値を投入
   (`spreadsheetId` / `reportEmail` / `geminiApiKey` / `geminiModel` / `geminiIncludeTaskNotes` / `morningReportTaskLimit`)
7. `clasp deploy --description "initial web app"` で Web アプリとしてデプロイ

既存 Spreadsheet を使う場合は `connectTaskLoggerSpreadsheet('YOUR_SPREADSHEET_ID')` を実行。

### トラブルシュート

- `spreadsheetId が未設定です` → `initializeTaskLoggerProject()` を実行
- `Tasks is not defined` → Tasks API 高度なサービスを追加
- 他人が Web アプリを開けない → デプロイのアクセス設定を見直す

### 共有時のチェックリスト

- `apps_script/.clasp.json` に自分の `scriptId` が残っていない
- `apps_script/how2redeploy.txt` に自分の `deploymentId` が残っていない
- Script Properties / `settings` シートの秘密情報をエクスポートに含めない

</details>
