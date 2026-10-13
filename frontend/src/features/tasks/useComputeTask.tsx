import { useCallback, useEffect, useRef, useState } from "react";
import { desktop, type BackgroundTask, type SchedulerProgressEvent } from "../../bridge/desktop";
import { Button } from "../../components/ui/button";

export async function waitForComputeResult<T>(id: string, readResult: (id: string) => Promise<T | null>, signal?: AbortSignal, onProgress?: (task: SchedulerProgressEvent) => void): Promise<T> {
  let wake: (() => void) | undefined;
  // Event subscription can be unavailable during an IPC reconnect. Persisted
  // state remains the source of truth and is read by ID without listing history.
  let unlisten: (() => void) | undefined;
  const subscribe = async () => {
    if (unlisten) return;
    try {
      unlisten = await desktop.listenSchedulerProgress((task) => {
        if (task.id === id) { onProgress?.(task); wake?.(); }
      });
    } catch { /* Retry subscription on the next persisted-state reconciliation. */ }
  };
  await subscribe();
  const abort = () => wake?.();
  signal?.addEventListener("abort", abort);
  try {
    for (;;) {
      if (signal?.aborted) throw new DOMException("页面已关闭；后台任务继续运行", "AbortError");
      // Reconcile persisted state after reconnect or missed events. Elapsed time
      // is never interpreted as a task failure.
      let task: BackgroundTask;
      try { task = await desktop.getBackgroundTask(id); }
      catch (reason) {
        if (String(reason).includes("task_not_found") || String(reason).includes("invalid_task_id")) throw new Error("后台任务不存在");
        // A disconnect does not establish that a durable task failed.
        await new Promise<void>((resolve) => {
          const finish = () => { clearTimeout(timer); signal?.removeEventListener("abort", finish); resolve(); };
          const timer = setTimeout(finish, 3000);
          signal?.addEventListener("abort", finish, { once: true });
          if (signal?.aborted) finish();
        });
        continue;
      }
      if (!task || task.id !== id) throw new Error("后台任务不存在");
      await subscribe();
      onProgress?.(task);
      if (["failed", "cancelled", "interrupted"].includes(task.state)) throw new Error(`任务${task.state === "cancelled" ? "已取消" : "未完成"}：${task.error_code ?? task.state}`);
      if (task.state === "completed") {
        const completed = await readResult(id);
        if (completed != null) return completed;
        throw new Error("任务已完成，但结果不可用");
      }
      await new Promise<void>((resolve) => {
        const timer = setTimeout(() => { wake = undefined; resolve(); }, 3000);
        wake = () => { clearTimeout(timer); wake = undefined; resolve(); };
        if (signal?.aborted) wake();
      });
    }
  } finally { unlisten?.(); signal?.removeEventListener("abort", abort); }
}

export function useComputeTask<T>(storageKey: string, readResult: (id: string) => Promise<T | null>) {
  const [task, setTask] = useState<SchedulerProgressEvent | null>(null);
  const [result, setResult] = useState<T | null>(null);
  const [error, setError] = useState("");
  const controller = useRef<AbortController | null>(null);
  const run = useCallback(async (id: string) => {
    controller.current?.abort();
    const next = new AbortController(); controller.current = next;
    localStorage.setItem(storageKey, id); setResult(null); setError("");
    try {
      const value = await waitForComputeResult(id, readResult, next.signal, (update) => { if (!next.signal.aborted) setTask(update); });
      if (!next.signal.aborted) { setResult(value); setTask((t) => t ? { ...t, state: "completed", progress: 100 } : t); localStorage.removeItem(storageKey); }
      return value;
    } catch (reason) {
      if (!next.signal.aborted) setError(reason instanceof Error ? reason.message : String(reason));
      throw reason;
    }
  }, [storageKey, readResult]);
  useEffect(() => {
    setTask(null); setResult(null); setError("");
    const id = localStorage.getItem(storageKey);
    if (id) void run(id).catch(() => undefined);
    return () => { controller.current?.abort(); };
  }, [run, storageKey]);
  return { task, result, error, run,
    cancel: async () => { if (task) await desktop.cancelBackgroundTask(task.id); },
    retry: async () => { if (task) { const next: BackgroundTask = await desktop.retryBackgroundTask(task.id); return run(next.id); } },
  };
}

export function ComputeTaskStatus({ task, error, cancel, retry }: { task: SchedulerProgressEvent | null; error: string; cancel: () => Promise<unknown>; retry: () => Promise<unknown> }) {
  const [actionError, setActionError] = useState("");
  const action = async (fn: () => Promise<unknown>) => { try { setActionError(""); await fn(); } catch (e) { setActionError(e instanceof Error ? e.message : String(e)); } };
  if (!task && !error) return null;
  return <div role="status"><span>{task ? `后台任务 ${task.state} · ${task.progress}%` : ""} {error || actionError}</span>
    {task && ["queued", "running", "waiting_external", "paused"].includes(task.state) && <Button variant="ghost" onClick={() => void action(cancel)}>取消任务</Button>}
    {task && ["failed", "cancelled", "interrupted", "paused"].includes(task.state) && <Button variant="ghost" onClick={() => void action(retry)}>重试任务</Button>}
  </div>;
}
