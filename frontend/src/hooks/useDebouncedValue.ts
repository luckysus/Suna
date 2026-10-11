import { useEffect, useState } from "react";

/**
 * 第 76 章：把频繁变化的值延迟到停顿后再提交（搜索框、筛选条件等）。
 *
 * ```ts
 * const debouncedQuery = useDebouncedValue(query, 180);
 * ```
 */
export function useDebouncedValue<T>(value: T, delay = 200): T {
  const [debounced, setDebounced] = useState(value);

  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(value), Math.max(0, delay));
    return () => window.clearTimeout(timer);
  }, [value, delay]);

  return debounced;
}
