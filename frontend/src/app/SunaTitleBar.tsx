import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

function Glyph({ d }: { d: string }) {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true" shapeRendering="crispEdges">
      <path d={d} fill="none" stroke="currentColor" strokeWidth="1" />
    </svg>
  );
}

export default function SunaTitleBar() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const appWindow = getCurrentWindow();
    let disposed = false;
    const sync = () => {
      void appWindow.isMaximized().then((value) => {
        if (!disposed) setMaximized(value);
      }).catch(() => undefined);
    };
    sync();
    const unlisten = appWindow.onResized(sync);
    return () => {
      disposed = true;
      void unlisten.then((off) => off()).catch(() => undefined);
    };
  }, []);

  return (
    <div className="suna-titlebar" data-tauri-drag-region>
      <button type="button" className="suna-titlebar-button" aria-label="最小化" title="最小化" onClick={() => void getCurrentWindow().minimize()}>
        <Glyph d="M0 5h10" />
      </button>
      <button
        type="button"
        className="suna-titlebar-button"
        aria-label={maximized ? "向下还原" : "最大化"}
        title={maximized ? "向下还原" : "最大化"}
        onClick={() => void getCurrentWindow().toggleMaximize()}
      >
        <Glyph d={maximized ? "M2.5 0.5h7v7M0.5 2.5h7v7h-7z" : "M0.5 0.5h9v9h-9z"} />
      </button>
      <button type="button" className="suna-titlebar-button is-close" aria-label="关闭" title="关闭" onClick={() => void getCurrentWindow().close()}>
        <Glyph d="M0 0l10 10M10 0L0 10" />
      </button>
    </div>
  );
}
