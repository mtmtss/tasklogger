import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { addCapture } from "../lib/commands";

/**
 * クイックキャプチャ窓 (AI 拡張仕様 §13.3)。
 * グローバルホットキー / トレイから開き、Enter で即ローカル保存して閉じる。
 * フォーカスされたテキストエリアに Win+H でそのまま音声入力できる。
 */
export default function CaptureWindow() {
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // 窓が再表示されたとき (show + focus) に入力欄へフォーカスを戻す
  useEffect(() => {
    const win = getCurrentWindow();
    const unlisten = win.onFocusChanged(({ payload: focused }) => {
      if (focused) inputRef.current?.focus();
    });
    return () => {
      void unlisten.then((f) => f());
    };
  }, []);

  const close = () => {
    setText("");
    setError(null);
    void getCurrentWindow().hide();
  };

  const save = async () => {
    if (!text.trim()) {
      close();
      return;
    }
    try {
      await addCapture(text);
      close();
    } catch (e) {
      setError(String(e));
    }
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void save();
    } else if (e.key === "Escape") {
      close();
    }
  };

  return (
    <div className="flex h-screen select-none flex-col gap-2 rounded-xl border border-slate-700 bg-slate-900/95 p-3 text-slate-100">
      <div
        data-tauri-drag-region
        className="flex items-center justify-between text-xs text-slate-500"
      >
        <span data-tauri-drag-region>
          思いつきをメモ — Enter 保存 / Shift+Enter 改行 / Esc 閉じる / Win+H
          音声入力
        </span>
        <button
          onClick={close}
          title="閉じる"
          className="rounded px-1.5 text-slate-500 hover:bg-slate-800 hover:text-slate-300"
        >
          ✕
        </button>
      </div>
      <textarea
        ref={inputRef}
        autoFocus
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={onKeyDown}
        placeholder="タスク・アイデア・メモを何でも。分類は後で AI が提案します"
        className="flex-1 resize-none rounded-md bg-slate-800 p-2 text-sm outline-none placeholder:text-slate-500 focus:ring-1 focus:ring-sky-600"
      />
      {error && <p className="text-xs text-rose-400">{error}</p>}
    </div>
  );
}
