import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./App";
import FloatWindow from "./routes/FloatWindow";
import CaptureWindow from "./routes/CaptureWindow";
import "./index.css";

const queryClient = new QueryClient();

// window label でエントリを分岐する
// (main = メインページ / float = フロートウィジェット / capture = クイックキャプチャ)
const label = getCurrentWindow().label;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      {label === "float" ? (
        <FloatWindow />
      ) : label === "capture" ? (
        <CaptureWindow />
      ) : (
        <App />
      )}
    </QueryClientProvider>
  </React.StrictMode>,
);
