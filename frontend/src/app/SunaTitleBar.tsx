import { useEffect, useState } from "react";
import { desktop } from "../bridge/desktop";
import { useLocale } from "../i18n/locale";

function Glyph({ d }: { d: string }) {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true" shapeRendering="crispEdges">
      <path d={d} fill="none" stroke="currentColor" strokeWidth="1" />
    </svg>
  );
}

export default function SunaTitleBar() {
  const { t } = useLocale();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let disposed = false;
    const sync = () => {
      void desktop.isMainWindowMaximized().then((value) => {
        if (!disposed) setMaximized(value);
      }).catch(() => undefined);
    };
    sync();
    let unlisten: (() => void) | undefined;
    void desktop.onMainWindowResized(sync).then((off) => {
      if (disposed) off();
      else unlisten = off;
    }).catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return (
    <div className="suna-titlebar" data-tauri-drag-region>
      <button type="button" className="suna-titlebar-button" aria-label={t("windowMinimize")} title={t("windowMinimize")} onClick={() => void desktop.minimizeMainWindow()}>
        <Glyph d="M0 5h10" />
      </button>
      <button
        type="button"
        className="suna-titlebar-button"
        aria-label={maximized ? t("windowRestore") : t("windowMaximize")}
        title={maximized ? t("windowRestore") : t("windowMaximize")}
        onClick={() => void desktop.toggleMainWindowMaximize()}
      >
        <Glyph d={maximized ? "M2.5 0.5h7v7M0.5 2.5h7v7h-7z" : "M0.5 0.5h9v9h-9z"} />
      </button>
      <button type="button" className="suna-titlebar-button is-close" aria-label={t("windowClose")} title={t("windowClose")} onClick={() => void desktop.closeMainWindow()}>
        <Glyph d="M0 0l10 10M10 0L0 10" />
      </button>
    </div>
  );
}
