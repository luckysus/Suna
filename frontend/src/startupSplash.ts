import { desktop, isDesktopRuntime } from "./bridge/desktop";

/**
 * 启动动画平滑过渡与注销机制。
 *
 * 保证在 React 根组件（SunaAppShell）真正挂载且双帧绘制上屏后，
 * 再平滑淡出并移除 index.html 预置的 #suna-boot-screen，
 * 避免因 React 异步渲染首帧未落位造成的界面闪烁。
 */

let dismissed = false;

export function markBoot(stage: string): void {
  if (!isDesktopRuntime()) return;
  void desktop.reportBootTiming(stage, Math.round(performance.now())).catch(() => undefined);
}

export function dismissBootScreen(): void {
  if (dismissed || typeof document === "undefined") return;
  const bootScreen = document.getElementById("suna-boot-screen");
  if (!bootScreen) {
    dismissed = true;
    return;
  }
  dismissed = true;
  bootScreen.classList.add("is-leaving");
  setTimeout(() => {
    bootScreen.remove();
  }, 450);
}
