import { useEffect, useState } from "react";
import { desktop, isDesktopRuntime } from "../../../bridge/desktop";
import OllamaModelDownload from "./OllamaModelDownload";

export default function ModelOllamaSettings() {
  const [baseUrl, setBaseUrl] = useState("http://127.0.0.1:11434");
  const [tasks, setTasks] = useState<Awaited<ReturnType<typeof desktop.listOllamaDownloadTasks>>>([]);

  useEffect(() => {
    if (!isDesktopRuntime()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;

    const refreshTasks = () => {
      void desktop
        .listOllamaDownloadTasks()
        .then((list) => {
          if (!disposed && Array.isArray(list)) setTasks(list);
        })
        .catch(() => undefined);
    };

    refreshTasks();
    void desktop
      .listenOllamaDownloadProgress(() => {
        refreshTasks();
      })
      .then((dispose) => {
        if (disposed) {
          dispose();
          return;
        }
        unlisten = dispose;
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return (
    <section className="suna-ollama-settings" aria-labelledby="ollama-settings-title">
      <h3 id="ollama-settings-title">Ollama 本地模型</h3>
      <p className="suna-settings-field-hint">管理本机 Ollama 服务地址、已安装模型与拉取任务。</p>
      <label className="suna-settings-field">
        <span>服务地址</span>
        <input value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} />
      </label>
      <OllamaModelDownload baseUrl={baseUrl} onSelectModel={() => undefined} />
      {tasks.length > 0 && (
        <ul className="suna-ollama-task-list">
          {tasks.map((task) => (
            <li key={task.id}>
              <strong>{task.model_name}</strong>
              <span>{task.status}</span>
              <span>{Math.round((task.progress ?? 0) * 100)}%</span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
