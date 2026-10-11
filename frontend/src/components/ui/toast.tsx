import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { CheckCircle2, Info, X, XCircle } from "lucide-react";
import { cn } from "../../lib/utils";
import "./toast.css";

/**
 * 第 63 章组件规范：Toast。
 *
 * 轻量通知系统：`ToastProvider` 挂在应用根部，任意组件用 `useToast()` 推送
 * 成功 / 提示 / 失败消息，自动消失、可手动关闭，不阻塞交互。
 */

export type ToastTone = "info" | "success" | "error";

export type ToastInput = {
  tone?: ToastTone;
  title: string;
  description?: string;
  /** 毫秒；0 表示不自动关闭。 */
  duration?: number;
};

type ToastRecord = Required<Pick<ToastInput, "tone" | "title" | "duration">> & {
  id: string;
  description?: string;
};

type ToastContextValue = {
  push: (toast: ToastInput) => string;
  dismiss: (id: string) => void;
  clear: () => void;
};

const ToastContext = createContext<ToastContextValue | null>(null);

const TONE_ICON: Record<ToastTone, typeof Info> = {
  info: Info,
  success: CheckCircle2,
  error: XCircle,
};

export function ToastProvider({ children, max = 4 }: { children: ReactNode; max?: number }) {
  const [toasts, setToasts] = useState<ToastRecord[]>([]);
  const timers = useRef(new Map<string, number>());
  const counter = useRef(0);

  const dismiss = useCallback((id: string) => {
    setToasts((current) => current.filter((toast) => toast.id !== id));
    const timer = timers.current.get(id);
    if (timer !== undefined) {
      window.clearTimeout(timer);
      timers.current.delete(id);
    }
  }, []);

  const push = useCallback(
    (toast: ToastInput) => {
      counter.current += 1;
      const id = `toast-${counter.current}`;
      const record: ToastRecord = {
        id,
        tone: toast.tone ?? "info",
        title: toast.title,
        description: toast.description,
        duration: toast.duration ?? 4200,
      };
      setToasts((current) => [...current, record].slice(-max));
      if (record.duration > 0) {
        const timer = window.setTimeout(() => dismiss(id), record.duration);
        timers.current.set(id, timer);
      }
      return id;
    },
    [dismiss, max],
  );

  const clear = useCallback(() => {
    setToasts([]);
    for (const timer of timers.current.values()) window.clearTimeout(timer);
    timers.current.clear();
  }, []);

  useEffect(() => () => {
    for (const timer of timers.current.values()) window.clearTimeout(timer);
    timers.current.clear();
  }, []);

  const value = useMemo<ToastContextValue>(() => ({ push, dismiss, clear }), [push, dismiss, clear]);

  return (
    <ToastContext.Provider value={value}>
      {children}
      <div className="suna-toast-viewport" role="region" aria-label="通知">
        {toasts.map((toast) => {
          const Icon = TONE_ICON[toast.tone];
          return (
            <div key={toast.id} className={cn("suna-toast", `is-${toast.tone}`)} role={toast.tone === "error" ? "alert" : "status"}>
              <Icon size={17} aria-hidden="true" />
              <div className="suna-toast-copy">
                <strong>{toast.title}</strong>
                {toast.description && <small>{toast.description}</small>}
              </div>
              <button type="button" className="suna-toast-close" onClick={() => dismiss(toast.id)} aria-label="关闭通知">
                <X size={14} aria-hidden="true" />
              </button>
            </div>
          );
        })}
      </div>
    </ToastContext.Provider>
  );
}

/** 取用通知能力；未挂载 `ToastProvider` 时返回空实现，调用方无需判空。 */
export function useToast(): ToastContextValue {
  const context = useContext(ToastContext);
  return context ?? { push: () => "", dismiss: () => undefined, clear: () => undefined };
}
