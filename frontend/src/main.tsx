import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource/inter/300.css";
import "@fontsource/inter/400.css";
import "@fontsource/inter/500.css";
import "@fontsource/inter/600.css";
import "@fontsource/inter/700.css";
import "@fontsource/inter/800.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
import "@fontsource/jetbrains-mono/700.css";
import "@fontsource/noto-sans-sc/400.css";
import "@fontsource/noto-sans-sc/500.css";
import "@fontsource/noto-sans-sc/700.css";
import SunaApp from "./app/SunaApp";
import { desktop, isDesktopRuntime } from "./bridge/desktop";
import { dismissBootScreen, markBoot } from "./startupSplash";
import "./index.css";

async function bootstrap() {
  markBoot("bootstrap-start");
  // Settings/theme providers read the desktop store during their first
  // effects. Initialise SQLite before mounting the tree so the first read
  // cannot race `db_init` and surface a false "database not initialized"
  // error on a fresh desktop launch.
  if (isDesktopRuntime()) {
    await desktop.initialize().catch(() => undefined);
  }
  markBoot("db-init-return");
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <SunaApp />
    </StrictMode>,
  );
  markBoot("render-called");

  // 兜底保护：若组件内部渲染因未捕获异常受阻，最晚 3 秒后移除启动画面
  setTimeout(() => {
    dismissBootScreen();
  }, 3000);
}

void bootstrap();
