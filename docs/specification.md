# TaskLogger (Tauri 版) 仕様書

- 版: 1.0
- 作成日: 2026-07-04
- 対象: TaskLogger の Tauri 製デスクトップアプリ版(GAS 版からの移行)
- 旧版仕様: [apps_script/specification.txt](../apps_script/specification.txt)(GAS 版、凍結)

---

# 1. 目的と背景

## 1.1 アプリの目的

Google Tasks に登録されているタスクのうち「今日やるべきタスク」を表示し、各タスクの作業開始・中断・完了を記録する時間管理アプリである。記録された作業ログはローカル DB に蓄積され、タスクリスト別・タスク別・期間別に作業時間を分析できる。

## 1.2 GAS 版からの移行理由

| 課題 | GAS 版 | Tauri 版での解決 |
| --- | --- | --- |
| 速度 | 全操作が Google への往復通信で、ボタン応答に数秒かかる | ローカルファースト: 全操作はローカル SQLite に即時反映(目標 100ms 以内)。Google への反映はバックグラウンド同期 |
| スリープ対応 | Web アプリのため PC スリープを検知できず、タイマーが回り続ける | OS の電源イベントを検知して自動中断 + 復帰時に再開ダイアログ |
| 常時表示 | ブラウザタブの中でしか見えない | 常に最前面のフロートウィンドウで現在タスクと経過時間を表示 |
| UI | 素の HTML + vanilla JS | React + TypeScript で全面刷新 |

## 1.3 スコープ外(GAS 版から移植しないもの)

- 朝レポート機能(メール送信 + Gemini 要約) — 移植しない
- Google Sheets への作業ログ保存 — ローカル SQLite に置き換え(将来拡張として Sheets 同期を検討)

---

# 2. アーキテクチャ概要

## 2.1 基本方針: ローカルファースト + Google Tasks 同期

```text
┌─ スマホ / 他デバイス ─┐        ┌──────── PC (本アプリ) ────────────┐
│  Google Tasks アプリ   │        │  React UI ── invoke ──> Rust コマンド │
└──────────┬───────────┘        │                            │        │
           │                     │                    SQLite (即時反映)  │
     Google Tasks API <──────────┤  同期ワーカー (pull / push キュー)     │
                                 └────────────────────────────────────┘
```

- **正本の定義**
  - タスク(何をやるか、期限、完了状態): **Google Tasks が正本**。スマホ等からは Google Tasks アプリで管理できる
  - 作業ログ(いつ・どれだけ作業したか): **ローカル SQLite が正本**。CSV エクスポートでバックアップ可能
- **全 UI 操作はローカル SQLite への書き込みで即座に完了**し、Google への反映(完了化・期限変更)はバックグラウンドの同期キューが担う。オフラインでも全機能が動作する

## 2.2 GAS 版から継承する不変条件

1. Google Tasks がタスクの multi-device 正本
2. 作業ログは**追記専用**。1 行 = 1 セッション(開始→中断 or 開始→完了)。削除・書き換えはしない
3. 同時に実行中(running)にできるタスクは **1 件のみ**(アクティブセッションはシングルトン)
4. タスクのアプリ内状態は**ログ + アクティブセッションから導出**する(状態を直接保存しない)
5. **完了時のみ** Google Tasks 側のタスクを完了に更新する。中断では更新しない
6. タイムゾーンは Asia/Tokyo。日時は ISO 8601 で保存し、`logDate` は startTime の JST 日付
7. ログにはタスクの ID と名称の両方を保存する(後からタスク名が変更されてもログは当時の名称を保持)

---

# 3. データモデル

DB ファイル: `{app_data_dir}/tasklogger.db`(Windows では `%APPDATA%/com.tasklogger.app/`)。
マイグレーションは rusqlite_migration で管理する。

## 3.1 task_lists / tasks(Google Tasks のローカルキャッシュ)

```sql
CREATE TABLE task_lists (
  id            TEXT PRIMARY KEY,        -- Google tasklist id
  title         TEXT NOT NULL,
  updated       TEXT,                    -- Google 側の updated
  deleted       INTEGER NOT NULL DEFAULT 0,
  fetched_at    TEXT NOT NULL
);

CREATE TABLE tasks (
  id            TEXT PRIMARY KEY,        -- Google task id
  task_list_id  TEXT NOT NULL REFERENCES task_lists(id),
  title         TEXT NOT NULL,
  notes         TEXT,
  due           TEXT,                    -- date-only RFC3339 (Google 仕様)
  status        TEXT NOT NULL,           -- 'needsAction' | 'completed'
  position      TEXT,
  updated       TEXT,
  deleted       INTEGER NOT NULL DEFAULT 0,
  dirty         INTEGER NOT NULL DEFAULT 0,  -- 未 push のローカル変更あり (pull 時に上書き禁止)
  fetched_at    TEXT NOT NULL
);
CREATE INDEX idx_tasks_due ON tasks(due, status);
```

## 3.2 work_logs(作業ログ、ローカル正本)

GAS 版 WorkLogs シートの 14 カラムと 1:1 互換 + 拡張 2 カラム。

```sql
CREATE TABLE work_logs (
  log_id           TEXT PRIMARY KEY,     -- UUID v4
  user_id          TEXT NOT NULL DEFAULT '',
  task_list_id     TEXT NOT NULL,
  task_list_name   TEXT NOT NULL,        -- 記録時点の名称 (改名耐性)
  task_id          TEXT NOT NULL,
  task_title       TEXT NOT NULL,
  action_type      TEXT NOT NULL CHECK (action_type IN ('paused','completed')),
  start_time       TEXT NOT NULL,        -- ISO 8601 UTC
  end_time         TEXT NOT NULL,
  duration_seconds INTEGER NOT NULL,
  duration_minutes INTEGER NOT NULL,     -- ceil(duration_seconds / 60)。GAS 版と同じ丸め
  log_date         TEXT NOT NULL,        -- JST yyyy-MM-dd (start_time 基準)
  memo             TEXT NOT NULL DEFAULT '',
  created_at       TEXT NOT NULL,
  -- 拡張カラム (Tauri 版で追加)
  end_reason       TEXT NOT NULL DEFAULT 'user',
                   -- 'user'            : ユーザーのボタン操作 (手動終了時の中断含む)
                   -- 'sleep'           : PC スリープによる自動中断
                   -- 'idle'            : 無操作 (ロック/スクリーンセーバ/離席) による自動中断
                   -- 'recovery'        : 異常終了からの起動時回復
                   -- 'direct_complete' : 中断中/未開始からの直接完了 (duration=0)
  source           TEXT NOT NULL DEFAULT 'app'   -- 'app' | 'import_gas'
);
CREATE INDEX idx_work_logs_log_date ON work_logs(log_date);
CREATE INDEX idx_work_logs_task ON work_logs(task_list_id, task_id);
```

### GAS WorkLogs との互換表

| GAS カラム | work_logs カラム | 備考 |
| --- | --- | --- |
| logId | log_id | UUID。インポート時の重複判定キー |
| userId | user_id | 単一ユーザー運用のため設定値から埋める(空可) |
| taskListId / taskListName | task_list_id / task_list_name | |
| taskId / taskTitle | task_id / task_title | |
| actionType | action_type | 'paused' \| 'completed' |
| startTime / endTime | start_time / end_time | ISO 8601 |
| durationSeconds / durationMinutes | duration_seconds / duration_minutes | 分は ceil 丸め |
| logDate | log_date | startTime の JST 日付 |
| memo | memo | |
| createdAt | created_at | |
| (なし) | end_reason / source | 拡張。CSV エクスポート時は末尾列に付加 |

## 3.3 active_session(実行中セッション、シングルトン)

```sql
CREATE TABLE active_session (
  id                INTEGER PRIMARY KEY CHECK (id = 1),  -- 常に 1 行のみ
  task_list_id      TEXT NOT NULL,
  task_list_name    TEXT NOT NULL,
  task_id           TEXT NOT NULL,
  task_title        TEXT NOT NULL,
  start_at          TEXT NOT NULL,       -- ISO 8601 UTC
  last_heartbeat_at TEXT NOT NULL        -- 30 秒毎に更新。スリープ/クラッシュ回復用 (§7)
);
```

## 3.4 sync_queue(Google への未送信操作)

```sql
CREATE TABLE sync_queue (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  op_type      TEXT NOT NULL,   -- 'complete_task' | 'set_due_today'
  task_list_id TEXT NOT NULL,
  task_id      TEXT NOT NULL,
  payload      TEXT NOT NULL,   -- JSON
  created_at   TEXT NOT NULL,
  attempts     INTEGER NOT NULL DEFAULT 0,
  last_error   TEXT
);
```

## 3.5 settings(key/value)

```sql
CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
```

主なキー: `float_window_position`, `autostart`, `close_to_tray`, `last_pull_at`, `last_full_pull_date`, `oauth_client_id`, `user_email`, `sleep_interrupted_task`(復帰ダイアログ用 JSON)。

---

# 4. タスクの状態管理

## 4.1 状態の定義

| 状態 | 説明 |
| --- | --- |
| not_started | 今日まだ作業していない |
| running | 現在タイマーが動いている(アクティブセッションあり) |
| paused | 今日作業したが中断中 |
| completed | 完了した |

状態は保存せず、**「今日の work_logs + active_session」から導出**する:

1. active_session が該当タスクなら `running`
2. そうでなければ、該当タスクの今日の最新ログ(end_time 順)の action_type が `completed` なら `completed`、`paused` なら `paused`
3. ログがなければ `not_started`

Google 側で完了済み(`status='completed'`)のタスクは今日リストに表示しない。Google 側で完了が取り消された場合は pull により再表示され、状態はログから再導出される(既存の completed ログは残す — 追記専用原則)。

## 4.2 状態遷移表(正式版)

| # | 現在状態 | 操作 / イベント | 次状態 | ログ書き込み | Google Tasks への反映 |
| --- | --- | --- | --- | --- | --- |
| 1 | not_started | 開始 | running | なし(セッション作成) | なし |
| 2 | running | 中断 | paused | `paused` 行 | なし |
| 3 | running | 完了 | completed | `completed` 行 | `complete_task` を enqueue |
| 4 | paused | 再開 | running | なし(セッション作成) | なし |
| 5 | **paused / not_started** | **完了(直接完了・新)** | completed | `completed` 行、start=end=now、**duration=0**、end_reason='direct_complete' | `complete_task` を enqueue |
| 6 | 候補タスク | **今すぐやる(新)** | running | なし(セッション作成) | due=today をローカル更新(dirty=1)+ `set_due_today` を enqueue |
| 7 | running | **PC スリープ(新)** | paused | `paused` 行、end_time=スリープ時刻、end_reason='sleep' | なし |
| 7b | running | **無操作しきい値超過(新)** ロック/スクリーンセーバ/離席 | paused | `paused` 行、end_time=最終入力時刻、end_reason='idle' | なし |
| 8 | running | アプリ終了(確認ダイアログで中断を選択) | paused | `paused` 行 | なし |
| 9 | running | クラッシュ / スリープ中の終了 → 次回起動 | paused | `paused` 行、end_time=last_heartbeat_at、end_reason='recovery' | なし |
| 10 | completed | (終端) | — | — | — |

**排他制御(#1, #4, #6 共通)**: 別のタスクが running の場合はエラー「現在作業中のタスクがあります。先に中断または完了してください。」を表示し、操作をブロックする(GAS 版と同じ挙動)。

## 4.3 直接完了の設計(新機能 1)

中断中(または未開始)のタスクを、再開せずにそのまま完了できる。

- 実装方式: **duration=0 の `completed` ログ行を追記する**(start_time = end_time = 現在時刻、end_reason='direct_complete')
- この方式を選ぶ理由:
  - 「状態はログから導出」原則を壊さない(導出ロジックが無変更で completed を返す)
  - 追記専用ログの監査性を保つ(いつ完了操作をしたかが行として残る)
  - 合計作業時間の集計に影響しない(duration=0)
- **集計ルール**: `duration_seconds = 0` かつ `end_reason = 'direct_complete'` の行は、合計時間には影響しないが**作業回数(セッション数)には数えない**

---

# 5. 機能仕様

## 5.1 今日やるリスト

- 表示対象: `due` が今日(JST)かつ未完了のタスク(ローカルキャッシュから取得。同期は裏で実行)
- タスクリスト別にグループ表示
- 各タスクの表示項目: タスクリスト名 / タスク名 / メモ(notes) / 状態 / 本日の累計作業時間
- ボタン表示ルール:

| 状態 | 表示するボタン |
| --- | --- |
| not_started | 開始、完了(直接完了) |
| running | 中断、完了 |
| paused | 再開、完了(直接完了) |
| completed | 操作不可(完了済み表示) |

## 5.2 開始・中断・再開・完了

- **開始 / 再開**: active_session を作成(start_at = 現在時刻)。別タスク running 中はブロック(§4.2)
- **中断**: `paused` ログを追記(duration = end - start)し、セッションを削除。任意でメモ入力可。Google 側は更新しない
- **完了**: `completed` ログを追記し、セッションを削除。任意でメモ入力可。`complete_task` を sync_queue に投入(Google 側を完了化)
- **直接完了**: §4.3 の通り

## 5.3 候補リストと「今すぐやる」(新機能 2)

- 候補 = due が今日でない(または due なし)未完了タスク
- GAS 版の「今日やる」ボタン(due=today に設定するだけ)を「**今すぐやる**」ボタンに変更。押下時の処理(1 トランザクション):
  1. 別タスクが running ならエラーでブロック(何も変更しない)
  2. 対象タスクの `due` をローカルで today に更新し `dirty=1`
  3. `set_due_today` を sync_queue に投入
  4. active_session を作成(即 running)
  5. `session-changed` / `tasks-changed` イベントを全ウィンドウへ発火

## 5.4 今日ダッシュボード

- サマリーカード: 今日のタスク数 / 今日の合計作業時間(実行中の経過分を含む) / 現在の経過時間 / 中断中タスク数
- タスクリスト別集計(合計時間 + 割合バー)
- 実行中タスクパネル: タスク名、開始時刻、経過時間タイマー(毎秒更新、フロントエンドで start_at から計算)、メモ欄、中断/完了ボタン

## 5.5 アーカイブ(期間分析)

- 期間指定: 開始日・終了日、プリセット(7 日 / 30 日 / 90 日)。デフォルト直近 30 日
- サマリー: 合計時間 / 日平均 / 稼働日数 / セッション数 / 完了数
- 日別推移(0 分の日も含む)、タスクリスト別集計、タスク別集計(最終作業日付き)
- グラフ表示(Recharts): 日別推移の棒グラフ、タスクリスト別の内訳
- 集計対象は work_logs のみ(direct_complete 行は §4.3 のルールで扱う)

## 5.6 CSV エクスポート / GAS データインポート

- **エクスポート**: work_logs を GAS WorkLogs 互換の列順 + 拡張 2 列で CSV 出力(期間指定可)
- **インポート**: GAS 版 Spreadsheet の WorkLogs シートを CSV で保存したものを設定画面から取り込み
  - `logId` が既存と重複する行はスキップ(GAS 版の重複防止思想を継承)
  - 取り込んだ行は `source='import_gas'`、`end_reason='user'`
  - 旧スキーマ(legacy 列順)も列名ヘッダで自動判別して受け入れる

---

# 6. Google 同期仕様

## 6.1 認証(OAuth 2.0)

- 方式: installed-app flow(**PKCE + loopback リダイレクト**)。`tauri-plugin-oauth` が 127.0.0.1 のランダムポートでコードを受領し、oauth2 crate でトークン交換
- スコープ: `https://www.googleapis.com/auth/tasks` のみ
- トークン保管: **refresh token は OS キーチェーン(Windows Credential Manager、keyring crate)**。access token はメモリのみ。SQLite には保存しない
- OAuth クライアント: 利用者が自分の Google Cloud プロジェクトで「デスクトップアプリ」用クライアントを作成し、設定画面に client_id を入力する(GAS 版で各自 Apps Script プロジェクトを作ったのと同様の各自セットアップ)。ビルド時埋め込みによる既定値も設定可能にする
- セットアップ手順(README にも記載):
  1. Google Cloud Console でプロジェクト作成(既存でも可)
  2. Google Tasks API を有効化
  3. OAuth 同意画面を作成(テストユーザーに自分を追加)
  4. 認証情報 → OAuth クライアント ID → 種類「デスクトップアプリ」で作成
  5. client_id を本アプリの設定画面に入力し「Google と接続」

## 6.2 Pull(Google → SQLite)

- トリガ: 起動時 / メインウィンドウフォーカス時(60 秒デバウンス)/ 5 分周期 / 手動同期ボタン / push 完了直後 / スリープ復帰時
- 手順: `tasklists.list` → 各リストで `tasks.list(showCompleted=false, showHidden=true, updatedMin=last_pull_at−5分)` の増分取得。1 日 1 回(または増分取得失敗時)はフル取得して削除検出
- 反映ルール: `dirty=1` の行は**上書きしない**(未送信のローカル変更を優先)。それ以外は Google 値で上書き(タスクは Google が正本)
- リモートで完了されたタスクはローカルも completed になり今日リストから消える。作業ログはそのまま残る

## 6.3 Push(SQLite → Google)

- すべての Google 書き込みは sync_queue 経由。バックグラウンドワーカー(tokio task)が FIFO で処理し、指数バックオフ(1s → 2s → … 最大 5 分)でリトライ。成功時に該当タスクの `dirty=0`
- 操作は 2 種のみ(いずれも冪等、at-least-once で安全):
  - `complete_task`: `tasks.patch { status:'completed', completed:now }`
  - `set_due_today`: `tasks.patch { due:'{today}T00:00:00.000Z' }`
- 競合処理:
  - 404(リモートで削除済み) → キューから破棄し通知
  - その他 4xx → キュー破棄 + エラー表示
  - 5xx / ネットワークエラー → リトライ継続

## 6.4 オフライン動作

- タスク開始/中断/完了/直接完了/集計はすべてオフラインで動作(ローカル完結)
- Google への反映はキューに滞留し、オンライン復帰で自動排出
- UI に同期状態(最終同期時刻 / キュー滞留数)をバッジ表示

## 6.5 UI 即応性の原則

- タスク操作の Tauri コマンドは **SQLite 更新のみ行い即 return**(目標 100ms 以内)
- Rust 側が `session-changed` / `tasks-changed` イベントを全ウィンドウへ emit し、フロントは TanStack Query の invalidate と Zustand store 更新で反映
- 同期は絶対に UI 操作をブロックしない

## 6.6 作業ログの Sheets 同期(複数デバイス統合)

複数 PC の作業ログを 1 つの Google スプレッドシートに合流させ、各 PC のアーカイブで統合集計を見られるようにする。

- **正本は引き続き各 PC のローカル SQLite**。Sheets は append-only の合流点であり、集計は常にローカル DB から計算する(UI 速度に影響しない)
- 同期は既存の 5 分周期ワーカーの 1 ステップとして実行(タスク同期の成否とは独立)。全処理がバックグラウンドスレッドで、UI 操作を一切ブロックしない
- **シート形式**: `WorkLogs` シート、GAS 版 v2 と同じ 14 列 + `endReason` / `source` の拡張 2 列。GAS 版の既存スプレッドシートの ID を指定すればそのまま合流できる(拡張 2 列のヘッダは自動追加、既存データ行は変更しない)
- **差分同期(logId ベース)**:
  - Pull: シートにあってローカルにない行を取込(`INSERT OR IGNORE`、`source='sheet_pull'`)
  - Push: ローカルにあってシートにない行を `values.append` で追記
  - logId が一意キーのため、どのデバイスから何度実行しても重複しない(冪等)
- **設定**: `sheet_sync_enabled`(既定 false)/ `log_spreadsheet_id`(空欄なら「TaskLogger Logs」を自動作成して保存)/ `last_sheet_sync_at`
- **OAuth スコープ**: `auth/tasks` に加えて `auth/spreadsheets` が必要。スコープ追加後は再接続(再同意)が必要
- 旧 legacy スキーマ(12 列)のシートは列位置が異なるため同期先として拒否し、CSV インポートを案内する

---

# 7. 無操作・スリープ・異常終了時の動作(新機能 3)

## 7.1 無操作検知(主検知、Windows)

**「一定時間 PC の操作がない」ことを中断のトリガーとする。** スリープ・画面ロック・スクリーンセーバ・離席を単一の仕組みでカバーする。

- `GetLastInputInfo`(user32)で最後のキーボード/マウス入力からの経過秒をハートビートループ(15 秒毎)で監視する
- running セッションがあり、無操作時間が **しきい値(設定 `idle_pause_minutes`、デフォルト 5 分、0 で無効)** を超えたら自動中断:
  - `paused` ログ追記。**end_time = 最後に入力があった時刻**(無操作分を作業時間に含めない)、end_reason='idle'
  - `sleep_interrupted_task` にタスク情報を記録し、セッション削除
- `GetTickCount` はスリープ中も進むため、スリープを挟いだ無操作も正しく計測される
- ユーザーの入力が再開した時点(無操作時間がしきい値超 → リセット)で §7.2 の再開ダイアログを表示する

## 7.1b スリープ即時検知(補助)

- `PowerRegisterSuspendResumeNotification`(powrprof、DEVICE_NOTIFY_CALLBACK、手書き FFI)を登録
  - コールバック型でメッセージループ不要、**Modern Standby(S0 low-power idle)でも通知される**
  - `PBT_APMSUSPEND`(スリープ突入)で即時中断(end_reason='sleep')、`PBT_APMRESUMEAUTOMATIC` / `PBT_APMRESUMESUSPEND` で復帰処理
  - 操作中に明示的にスリープさせた場合など、無操作しきい値を待たず正確なスリープ時刻で締められる
- **競合対策**: suspend コールバックの処理が suspend 前に完了しなかった場合、復帰処理の冒頭で「セッションが残っていてハートビートが 90 秒超古い」ことを検出し、ハートビート時刻で締める
- **二次(保険)**: ハートビートの壁時計ギャップ > 90 秒を「検知漏れスリープ」とみなして同処理(通知取りこぼし・強制休止・ハイバネートも吸収)
- OS 依存部は `platform/` に隔離(将来 macOS 対応時はここに実装を追加)

## 7.2 イベントフロー

```text
[無操作しきい値超過 / スリープ突入]
  running セッションあり?
    yes → `paused` ログ追記 (end_time=最終入力時刻 or スリープ時刻, end_reason='idle' | 'sleep')
        → settings.sleep_interrupted_task にタスク情報を記録
        → セッション削除

[入力再開 / スリープ復帰]
  → pull 同期をトリガ
  → sleep_interrupted_task があれば全ウィンドウへ `power-resumed` イベント
  → メインウィンドウにダイアログ表示:
      「中断していたタスク『◯◯』を再開しますか？」
      [再開する]   → 通常の再開と同一コマンドで running 化 → sleep_interrupted_task クリア
      [今はしない] → sleep_interrupted_task クリアのみ (paused のまま今日リストに残る)
```

## 7.3 起動時回復(クラッシュ / スリープ中の終了)

起動時、UI 表示前に実行:

1. `active_session` に行が残っている(= 前回正常にクローズしていない)場合
2. `paused` ログを追記する。**end_time = last_heartbeat_at**(実際に動いていた最後の時刻。スリープ後の時間水増しを防ぐ)、end_reason='recovery'
3. セッションを削除し、§7.2 と同じ「再開しますか？」ダイアログを表示

## 7.4 通常終了時

running セッションがある状態で終了操作(トレイ → 終了)をした場合、「作業中のタスクを中断して終了しますか？」を確認し、`paused` ログを書いてから終了する(黙って終了して回復処理任せにしない)。

---

# 8. 画面仕様

## 8.1 ウィンドウ構成

| ウィンドウ | label | 内容 |
| --- | --- | --- |
| メイン | main | `/today`(今日) `/archive`(アーカイブ) `/settings`(設定) の 3 ページ |
| フロート | float | 常時最前面ミニウィジェット(§8.3) |

React は単一バンドルで、window label によって初期ルートを分岐する。

## 8.2 メインウィンドウ

- **今日ページ**(起動時表示): 今日の日付 / サマリーカード / 実行中タスクパネル / 今日やるリスト(タスクリスト別グループ) / 候補リスト(「今すぐやる」ボタン付き) / 同期状態バッジ + 手動同期ボタン
- **アーカイブページ**: §5.5 の期間分析 + グラフ
- **設定ページ**: Google 接続(OAuth) / GAS データインポート / CSV エクスポート / 自動起動 / 閉じるボタンの挙動(トレイ常駐 or 終了) / フロートウィンドウ表示切替
- メインウィンドウの「閉じる」は既定でトレイ常駐(設定で変更可)

## 8.3 フロートウィンドウ(新機能 4)

- ウィンドウ設定: `decorations:false, alwaysOnTop:true, skipTaskbar:true, resizable:false, transparent:true`、サイズ約 340×92px
- 起動時に非表示で生成しておき、show/hide でトグル(生成/破棄はしない)。トグル手段: メインウィンドウのボタン / トレイメニュー
- 表示内容:
  - 実行中: タスク名(省略表示) / タスクリスト名 / 経過タイマー(毎秒更新) / [中断] [完了] ボタン
  - アイドル時: 「作業中タスクなし」+ メインウィンドウを開くボタン
- ドラッグ移動: ルート要素に `data-tauri-drag-region`(ボタン部分は除外)。位置は移動終了時に settings へ保存し、次回起動時に復元
- 状態共有: 真実は Rust 側 AppState(SQLite と同期)。フロート窓のボタンもメイン窓と同じ Tauri コマンドを invoke するだけ。状態変化は `session-changed` イベントで全ウィンドウに配信され、タイマーは各ウィンドウが start_at から独立計算する(毎秒の IPC なし)

## 8.4 トレイ

- 左クリック: メインウィンドウ表示
- メニュー: フロートウィンドウ表示切替 / 手動同期 / 終了

## 8.5 UI 刷新方針(新機能 5)

- Tailwind CSS + shadcn/ui(Radix)ベースのモダンなダークテーマ(GAS 版のダークブルー基調を継承しつつ再設計)
- 状態カラー: running=ブルー、paused=アンバー、completed=グリーン、not_started=グレー(GAS 版踏襲)
- トースト通知(操作結果・同期エラー)、スケルトンローディング
- グラフは Recharts

---

# 9. 技術スタックとプロジェクト構成

## 9.1 技術スタック

| 領域 | 選定 |
| --- | --- |
| フレームワーク | Tauri 2.x(tray はコア機能) |
| DB | rusqlite(bundled) + rusqlite_migration ※ tauri-plugin-sql は不採用(ビジネスロジックを Rust コマンド内に置くため) |
| HTTP / OAuth | reqwest(rustls-tls) / oauth2 crate(PKCE) + tauri-plugin-oauth + tauri-plugin-opener |
| トークン保管 | keyring crate(Windows Credential Manager) |
| 電源イベント | windows crate(Win32_System_Power) |
| その他 Rust | chrono + chrono-tz / uuid / serde / thiserror / tracing / tokio |
| Tauri プラグイン | single-instance(必須) / dialog / notification / autostart / window-state(メイン窓のみ) |
| フロント基盤 | React 18 + TypeScript + Vite |
| サーバ状態 | TanStack Query v5(invoke ラッパ経由) |
| クライアント状態 | Zustand(session store、`session-changed` 購読) |
| ルーティング | react-router(`/today` `/archive` `/settings` `/float`) |
| スタイル | Tailwind CSS + shadcn/ui |
| グラフ | Recharts |

## 9.2 プロジェクト構成

```text
tasklogger/
├── apps_script/            # 旧 GAS 版 (凍結・参照用)
├── docs/
│   └── specification.md    # 本仕様書
├── README.md
└── app/                    # Tauri アプリ
    ├── package.json / vite.config.ts / tailwind 設定 / tsconfig.json
    ├── src/                        # React
    │   ├── main.tsx                # window label → ルート分岐
    │   ├── routes/
    │   │   ├── TodayPage.tsx
    │   │   ├── ArchivePage.tsx
    │   │   ├── SettingsPage.tsx
    │   │   └── FloatWindow.tsx
    │   ├── components/             # TaskCard, RunningPanel, CandidateList,
    │   │                           # TimerDisplay, SyncBadge, ResumeDialog, charts/ ...
    │   ├── stores/sessionStore.ts  # Zustand
    │   ├── lib/commands.ts         # invoke ラッパ (型付き)
    │   ├── lib/queries.ts          # TanStack Query hooks
    │   └── types/                  # TaskItem, WorkLog, Summary
    └── src-tauri/
        ├── tauri.conf.json         # main + float の 2 window 定義
        ├── capabilities/           # window 毎の permission
        └── src/
            ├── main.rs / lib.rs    # setup: DB, 回復処理, PowerMonitor, tray, 同期ワーカー
            ├── commands/           # session.rs, tasks.rs, logs.rs, analytics.rs,
            │                       # sync.rs, auth.rs, settings.rs, import_export.rs
            ├── domain/             # state_machine.rs (遷移表の実装), models.rs, time.rs
            ├── db/                 # connection.rs, migrations/, repos/
            ├── google/             # auth.rs, tasks_api.rs, sync_worker.rs
            ├── platform/           # power_monitor.rs (trait), windows.rs, heartbeat.rs
            ├── float_window.rs
            └── tray.rs
```

## 9.3 主要 Tauri コマンド(API 面)

| コマンド | 内容 |
| --- | --- |
| `get_today_dashboard()` | 今日リスト + 状態 + 累計時間 + サマリー(ローカルから即時) |
| `get_candidates()` | 候補タスク一覧 |
| `start_task(task_ref)` | 開始/再開。running 排他チェック |
| `stop_task(action, memo)` | 中断('paused') / 完了('completed') |
| `complete_task_direct(task_ref)` | 直接完了(duration=0 ログ) |
| `do_it_now(task_ref)` | 今すぐやる(due=today + 開始) |
| `get_archive_analytics(start, end)` | 期間集計 |
| `resume_interrupted()` / `dismiss_interrupted()` | 復帰ダイアログの応答 |
| `connect_google()` / `disconnect_google()` / `get_sync_status()` / `sync_now()` | 認証・同期 |
| `export_csv(range)` / `import_gas_csv(path)` | エクスポート / インポート |
| `get_settings()` / `set_setting(key, value)` | 設定 |
| `toggle_float_window()` | フロート窓表示切替 |

---

# 10. 非機能要件

- **性能**: 全タスク操作は UI 反映まで 100ms 以内(ローカル SQLite のみ)。同期は非同期でUIをブロックしない
- **タイムゾーン**: Asia/Tokyo 固定(定数)。日時は ISO 8601(UTC)で保存し、表示時に JST 変換。`log_date` は start_time の JST 日付(深夜跨ぎは開始日基準 — GAS 版踏襲)
- **データ一貫性**: work_logs は追記専用。アプリからの削除・更新機能は設けない。タスク操作(ログ追記 + セッション変更 + キュー投入)は 1 SQLite トランザクションで行う
- **二重起動防止**: tauri-plugin-single-instance で 2 個目の起動はメインウィンドウ表示に転送(セッション破壊防止)
- **セキュリティ**: refresh token は OS キーチェーンのみ。SQLite・設定ファイルに秘密情報を置かない
- **想定規模**: 今日のタスク数十件、ログ数万行(SQLite + インデックスで十分)

---

# 11. 実装マイルストーン

| M | 内容 | 完了条件(独立検証可能) |
| --- | --- | --- |
| M0 | 本仕様書のコミット + `app/` scaffold(Tauri2 + React + TS + Tailwind) | `cargo check` / `tsc` が通り、ウィンドウが開く |
| M1 | ローカルコア: SQLite スキーマ・マイグレーション、状態機械(新遷移含む)、今日リスト UI(モックデータ)、開始/中断/完了/直接完了、今日ダッシュボード | Google なしで全操作・ログ記録・集計が動く。状態遷移の単体テストが通る |
| M2 | Google 連携: OAuth(PKCE + keyring)、pull、push キュー、「今すぐやる」 | 実アカウントでタスク往復。オフライン→復帰でキュー排出 |
| M3 | スリープ検知 + 起動時回復 + フロートウィンドウ + トレイ | 実機スリープで paused ログ生成・復帰ダイアログ表示。フロート窓から操作可能 |
| M4 | アーカイブページ(集計 + グラフ)、CSV エクスポート、GAS データインポート、設定ページ | 旧データ込みで集計が GAS 版と一致 |
| M5 | 磨き込み: autostart、通知、UI 仕上げ、Windows インストーラ(NSIS/MSI) | 日常運用開始 |

---

# 12. 将来拡張

- ~~Google Sheets への作業ログ同期~~ → **実装済み (§6.6)**
- アプリ内でのタスク作成・編集(現状タスク作成は Google Tasks 側で行う)
- macOS 対応(PowerMonitor trait の実装追加)
- ポモドーロタイマー / タスクごとの予定時間と実績比較
- 週間・月間レポート
- 未完了タスクの翌日繰り越し提案
- (GAS 版仕様書 §15 の拡張候補を引き継ぐ)
