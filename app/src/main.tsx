import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./App";
import FloatWindow from "./routes/FloatWindow";
import "./index.css";

const queryClient = new QueryClient();

// window label でエントリを分岐する (main = メイン 3 ページ / float = フロートウィジェット)
const isFloat = getCurrentWindow().label === "float";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      {isFloat ? <FloatWindow /> : <App />}
    </QueryClientProvider>
  </React.StrictMode>,
);
