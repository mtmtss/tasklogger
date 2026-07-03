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

## ウィンドウ構成

- `main`: メインウィンドウ(`/today` `/archive` `/settings` を react-router で切替)
- `float`: 常時最前面のフロートウィジェット(window label により [src/main.tsx](src/main.tsx) でエントリ分岐)
