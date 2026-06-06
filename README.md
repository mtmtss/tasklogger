# TaskLogger_GAS

Google Tasks と Google Apps Script を使って、日々のタスク実行時間を記録・可視化する Web アプリです。

このリポジトリは、他者に共有しても個人情報や機密情報を含めない形を前提に整理しています。

配布対象の本体は `apps_script/` です。

## ディレクトリ構成

```text
apps_script/
  appsscript.json
  WebAppController.js
  ProjectSetupService.js
  TaskService.js
  LogService.js
  ReportService.js
  TodayPage.html
  ArchivePage.html
  .clasp.json
  .clasp.example.json
```

## 前提条件

- Google アカウント
- Node.js
- `clasp`

`clasp` は Google 公式の Apps Script CLI です。

インストール:

```bash
npm install -g @google/clasp
```

ログイン:

```bash
clasp login
```

公式:

- Apps Script + clasp: https://developers.google.com/apps-script/guides/clasp
- Advanced Google services: https://developers.google.com/apps-script/guides/services/advanced
- Tasks service: https://developers.google.com/apps-script/advanced/tasks

## 最短セットアップ手順

以下の手順で、初見でも再現できます。

### 1. リポジトリを取得

```bash
git clone <this-repository>
cd TaskLogger_GAS/apps_script
```

### 2. Apps Script プロジェクトを自分の Google アカウントで作成

```bash
clasp create --type standalone --title "TaskLogger_GAS"
```

このコマンドで `apps_script/.clasp.json` に自分の `scriptId` が入ります。

補足:

- このリポジトリにある `.clasp.json` はプレースホルダです
- 既存の `.clasp.json` を上書きして問題ありません

### 3. コードを Apps Script に反映

```bash
clasp push
```

### 4. Apps Script エディタで Google Tasks の高度なサービスを有効化

このプロジェクトは `Tasks` 高度なサービスを使います。

手順:

1. Apps Script エディタを開く
2. 左メニューの `Services` で `+` を押す
3. `Tasks API` を追加する

補足:

- `appsscript.json` には `Tasks` の設定が含まれています
- Apps Script のデフォルト Cloud プロジェクトを使う場合、通常はサービス追加時に API も有効化されます
- 標準の Google Cloud プロジェクトを明示的に紐付けている場合は、Cloud 側でも `Google Tasks API` を有効化してください

### 5. 初期化関数を 1 回実行

Apps Script エディタで `initializeTaskLoggerProject` を実行してください。

この関数は次を行います。

- Spreadsheet を新規作成して `spreadsheetId` を Script Properties に保存
- `WorkLogs` シートを初期化
- `ActiveSession` シートを初期化
- `settings` シートを初期化

初回実行時には認可ダイアログが表示されます。

### 6. 必要なら設定値を入れる

`initializeTaskLoggerProject()` 実行後、作成された Spreadsheet の `settings` シート、または Apps Script の Script Properties に以下を設定できます。

| key | required | description |
| --- | --- | --- |
| `spreadsheetId` | yes | 利用する Spreadsheet ID。通常は `initializeTaskLoggerProject()` が自動設定 |
| `reportEmail` | no | 朝レポート送信先メールアドレス |
| `geminiApiKey` | no | Gemini を使う場合の API キー |
| `geminiModel` | no | 既定値は `gemini-2.5-flash` |
| `geminiIncludeTaskNotes` | no | `true` でメモを Gemini に渡す |
| `morningReportTaskLimit` | no | 朝レポート対象タスク数 |

方針:

- 秘密情報は Script Properties を優先
- `settings` シートは非秘密設定向け

### 7. Web アプリとしてデプロイ

```bash
clasp deploy --description "initial web app"
```

既存デプロイ更新:

```bash
clasp deployments
clasp version "release YYYY-MM-DD"
clasp redeploy <deploymentId> --versionNumber <versionNumber> --description "release YYYY-MM-DD"
```

### 8. デプロイ設定

Web アプリの設定は用途で決めてください。

- 自分だけで使うなら: 実行ユーザー = 自分、アクセス = 自分のみ
- チーム共有するなら: 実行ユーザー = 自分、アクセス = Google アカウントを持つ全員 または ドメイン内

注意:

- Google Tasks は実行ユーザーの権限で動きます
- チーム全員がそれぞれ自分の Tasks を使う構成にしたい場合は、運用方針を先に決めてください

## Spreadsheet を既存のものにしたい場合

既存 Spreadsheet を使うなら、Apps Script エディタで次を実行します。

```javascript
connectTaskLoggerSpreadsheet('YOUR_SPREADSHEET_ID');
```

## 動作確認手順

最低限、次を確認してください。

1. Web アプリを開ける
2. 今日ページが表示される
3. アーカイブページへ遷移できる
4. Google Tasks の一覧が取得できる
5. タスク開始 → 中断/完了で `WorkLogs` に記録される
6. `initializeTaskLoggerProject()` で作成された Spreadsheet にシートがそろっている

## 共有時のチェックリスト

共有前に必ず確認してください。

- `apps_script/.clasp.json` に自分の `scriptId` が残っていない
- `apps_script/how2redeploy.txt` に自分の `deploymentId` が残っていない
- `Script Properties` の値を README に書いていない
- `settings` シートに個人のメールアドレスや API キーを書いたままエクスポートしていない
- Spreadsheet そのものを共有する場合、ログデータや個人情報が残っていない

## 既知の前提

- Google Tasks の高度なサービスが有効であること
- Spreadsheet は Apps Script からアクセス可能であること
- 朝レポートの Gemini 利用は任意
- Gemini を使わない場合でも、タスク管理とログ集計は動作します

## トラブルシュート

### `spreadsheetId が未設定です`

`initializeTaskLoggerProject()` を 1 回実行してください。

### `Tasks is not defined`

Apps Script エディタで `Tasks API` 高度なサービスを追加してください。

### Web アプリを開いても他人が使えない

デプロイのアクセス設定を見直してください。ソースコードではなく、デプロイ設定側の問題です。

### Gemini 関連で失敗する

`geminiApiKey` を設定していないか、キーの権限不足です。未設定ならフォールバックで動く設計です。
