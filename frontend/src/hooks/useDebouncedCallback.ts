import { useCallback, useEffect, useRef } from "react";

/**
 * 第 76 章：延迟执行回调，并在重复触发时重置计时（自动保存、持久化等）。
 *
 * ```ts
 * const { schedule, cancel } = useDebouncedCallback((next) => void persist(next), 650);
 * schedule(value);
 * ```
 */
export function useDebouncedCallback<Args extends unknown[]>(
  callback: (...args: Args) => void,
  delay = 650,
): { schedule: (...args: Args) => void; cancel: () => void; flush: (...args: Args) => void } {
  const callbackRef = useRef(callback);
  const timer = useRef<number | null>(null);
  const pending = useRef<Args | null>(null);

  useEffect(() => { callbackRef.current = callback; }, [callback]);

  const cancel = useCallback(() => {
    if (timer.current !== null) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
    pending.current = null;
  }, []);

  const schedule = useCallback((...args: Args) => {
    pending.current = args;
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      timer.current = null;
      const next = pending.current;
      pending.current = null;
      if (next) callbackRef.current(...next);
    }, Math.max(0, delay));
  }, [delay]);

  /** 立刻执行待处理的调用（例如用户点「保存」时）。 */
  const flush = useCallback((...args: Args) => {
    cancel();
    callbackRef.current(...args);
  }, [cancel]);

  useEffect(() => cancel, [cancel]);

  return { schedule, cancel, flush };
}
