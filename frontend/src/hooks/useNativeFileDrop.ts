import { useEffect, useRef, type RefObject } from "react";
import { desktop, isDesktopRuntime } from "../bridge/desktop";

/** Tauri supplies file paths through its native WebView event, in physical pixels. */
export function useNativeFileDrop<T extends HTMLElement>(
  region: RefObject<T | null>,
  options: {
    disabled?: boolean;
    onFiles: (paths: string[]) => void | Promise<void>;
    onHover?: (hovering: boolean) => void;
    onError?: (error: unknown) => void;
  },
) {
  const current = useRef(options);
  current.current = options;
  useEffect(() => {
    if (!isDesktopRuntime() || typeof desktop.listenFileDrop !== "function") return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void desktop.listenFileDrop((event) => {
      if (disposed) return;
      const { disabled, onFiles, onHover, onError } = current.current;
      if (event.type === "leave" || disabled) { onHover?.(false); return; }
      const bounds = region.current?.getBoundingClientRect();
      const ratio = window.devicePixelRatio || 1;
      const x = event.position.x / ratio;
      const y = event.position.y / ratio;
      const inside = !!bounds && x >= bounds.left && x <= bounds.right && y >= bounds.top && y <= bounds.bottom;
      onHover?.(inside && event.type !== "drop");
      if (inside && event.type === "drop") {
        void Promise.resolve().then(() => onFiles(event.paths)).catch((error) => onError?.(error));
      }
    }).then((release) => {
      if (disposed) release(); else unlisten = release;
    }).catch((error) => { if (!disposed) current.current.onError?.(error); });
    return () => { disposed = true; unlisten?.(); };
  }, [region]);
}
