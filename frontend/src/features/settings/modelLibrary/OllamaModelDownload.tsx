import { useEffect, useState } from "react";
import { Download, LoaderCircle } from "lucide-react";
import { desktop, isDesktopRuntime } from "../../../bridge/desktop";
import { settingsErrorMessage } from "../settingsError";
import { Button } from "../../../components/ui/button";
import { Input } from "../../../components/ui/input";

export default function OllamaModelDownload({
  baseUrl,
  onSelectModel,
}: {
  baseUrl: string;
  onSelectModel: (modelName: string) => void;
}) {
  const [reachable, setReachable] = useState<boolean | null>(null);
  const [models, setModels] = useState<string[]>([]);
  const [downloadName, setDownloadName] = useState("");
  const [progress, setProgress] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isDesktopRuntime()) return;
    let cancelled = false;
    void desktop.checkOllamaStatus(baseUrl || null).then((status) => {
      if (cancelled) return;
      setReachable(status.reachable);
      if (!status.reachable) {
        setError(status.message ?? "无法连接 Ollama");
      }
    }).catch(() => undefined);
    void desktop.listOllamaModels(baseUrl || null).then((tags) => {
      if (!cancelled && Array.isArray(tags)) {
        setModels(tags.map((tag) => tag.name));
      }
    }).catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [baseUrl]);

  useEffect(() => {
    if (!isDesktopRuntime()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void desktop
      .listenOllamaDownloadProgress((event) => {
        if (!event || typeof event.progress !== "number") return;
        setProgress(event.progress);
        if (event.status === "success" || event.progress >= 1) {
          setBusy(false);
          if (event.model_name) onSelectModel(event.model_name);
          void desktop.listOllamaModels(baseUrl || null).then((tags) => {
            if (Array.isArray(tags)) setModels(tags.map((tag) => tag.name));
          }).catch(() => undefined);
        }
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
  }, [baseUrl, onSelectModel]);

  const download = async () => {
    const name = downloadName.trim();
    if (!name || !isDesktopRuntime()) return;
    setBusy(true);
    setError(null);
    setProgress(0);
    try {
      await desktop.downloadOllamaModel(name, baseUrl || null);
    } catch (cause) {
      setError(settingsErrorMessage(cause, "Ollama 模型下载失败"));
      setBusy(false);
    }
  };

  return (
    <section className="suna-ollama-download" aria-label="Ollama 模型">
      <p className="suna-settings-field-hint">
        {reachable === null ? "正在检测 Ollama…" : reachable ? "已连接本地 Ollama" : "Ollama 未就绪"}
      </p>
      {models.length > 0 && (
        <div className="suna-model-catalog-models">
          {models.map((name) => (
            <Button key={name} type="button" variant="secondary" onClick={() => onSelectModel(name)}>
              {name}
            </Button>
          ))}
        </div>
      )}
      <div className="suna-ollama-download-row">
        <Input
          value={downloadName}
          onChange={(event) => setDownloadName(event.target.value)}
          placeholder="输入要拉取的模型名，如 qwen2.5:7b"
          aria-label="Ollama 模型名"
        />
        <Button type="button" variant="primary" disabled={busy || !downloadName.trim()} onClick={() => void download()}>
          {busy ? <LoaderCircle size={16} className="suna-spin" /> : <Download size={16} />}
          拉取
        </Button>
      </div>
      {busy && (
        <div className="suna-ollama-progress" role="progressbar" aria-valuenow={Math.round(progress * 100)}>
          <div style={{ width: `${Math.round(progress * 100)}%` }} />
        </div>
      )}
      {error && <p className="suna-settings-inline-error" role="alert">{error}</p>}
    </section>
  );
}
