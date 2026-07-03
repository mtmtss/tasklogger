# TaskLogger (Tauri 版)

Tauri 2 + React + TypeScript + Tailwind CSS のデスクトップアプリ。仕様は [../docs/specification.md](../docs/specification.md) を参照。

## 開発環境の前提

- Node.js 20.19+ / 22.12+ 推奨(Vite 7 の要件)
- Rust (stable, MSVC toolchain)
- Visual Studio Build Tools (C++ ワークロード)
- Windows 11 (WebView2 同梱)

## コマンド

```bash
npm install        # 依存インストール
npm run tauri dev  # 開発起動 (Vite + Tauri)
npm run build      # tsc + vite build (フロントのみ)
npm run tauri build # 配布ビルド
```

Rust 側のみの検証は `src-tauri/` で `cargo check`。

## Google 連携のセットアップ

タスクの取得・完了反映には Google Tasks API を使います (仕様書 §6)。初回のみ以下の各自セットアップが必要です。

1. [Google Cloud Console](https://console.cloud.google.com/) でプロジェクトを作成 (既存でも可)
2. 「API とサービス → ライブラリ」で **Google Tasks API** を有効化
3. 「OAuth 同意画面」を作成し、テストユーザーに自分の Google アカウントを追加
4. 「認証情報 → 認証情報を作成 → OAuth クライアント ID」で種類 **「デスクトップ アプリ」** を選んで作成
5. 表示されたクライアント ID / クライアント シークレットを、本アプリの **設定ページ** に入力して「Google と接続」

- refresh token は Windows 資格情報マネージャーに保存されます (SQLite には保存しません)
- 未接続でもアプリは全機能ローカルで動作します (同期のみ停止)

## ウィンドウ構成

- `main`: メインウィンドウ(`/today` `/archive` `/settings` を react-router で切替)
- `float`: 常時最前面のフロートウィジェット(window label により [src/main.tsx](src/main.tsx) でエントリ分岐)
