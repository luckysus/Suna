import { useEffect, useRef, useState } from "react";
import { Check, LoaderCircle, RotateCcw, SlidersHorizontal } from "lucide-react";
import { useLocale } from "../../i18n/locale";
import { settingsErrorMessage } from "./settingsError";
import { Input } from "../../components/ui/input";
import { loadModelRuntime, modelRuntimeDefaults, normalizeModelRuntime, saveModelRuntime, type ModelRuntimePreferences } from "./modelPreferences";
import { Button } from "../../components/ui/button";

const defaults = modelRuntimeDefaults;

export default function ModelRuntimeSettings() {
  const { t } = useLocale();
  const [value, setValue] = useState(defaults);
  const [state, setState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const requestRef = useRef(0);

  useEffect(() => {
    let mounted = true;
    void loadModelRuntime().then((parsed) => {
      if (mounted) setValue(parsed);
    }).catch((cause) => { if (mounted) { setState("error"); setError(settingsErrorMessage(cause, "无法读取模型运行参数")); } }).finally(() => { if (mounted) setLoading(false); });
    return () => { mounted = false; };
  }, []);

  const update = async (next: Partial<ModelRuntimePreferences>) => {
    const merged = normalizeModelRuntime({ ...value, ...next });
    const request = ++requestRef.current;
    setValue(merged);
    setError(null);
    setState("saving");
    try {
      await saveModelRuntime(merged);
      if (request === requestRef.current) {
        setState("saved");
        window.setTimeout(() => { if (request === requestRef.current) setState("idle"); }, 1200);
      }
    } catch (cause) {
      if (request === requestRef.current) {
        setState("error");
        setError(settingsErrorMessage(cause, "模型运行参数保存失败"));
      }
    }
  };

  const reset = () => { void update(defaults); };

  return (
    <section className="suna-settings-runtime" aria-labelledby="model-runtime-heading">
      <div className="suna-settings-runtime-heading">
        <div><span className="suna-settings-card-icon"><SlidersHorizontal size={16} aria-hidden="true" /></span><div><h2 id="model-runtime-heading">模型运行参数</h2><p>应用于新的 Agent 运行。Provider 和 API Key 仍按能力单独配置。</p></div></div>
        {loading && <span className="suna-settings-inline-saved" role="status"><LoaderCircle size={14} className="suna-spin" aria-hidden="true" />正在加载</span>}
        {!loading && state === "saving" && <span className="suna-settings-inline-saved" role="status"><LoaderCircle size={14} className="suna-spin" aria-hidden="true" />保存中</span>}
        {state === "saved" && <span className="suna-settings-inline-saved" role="status"><Check size={14} aria-hidden="true" />{t("settingsSaved")}</span>}
        {state === "error" && <span className="suna-settings-inline-error" role="alert">{error ?? "保存失败"}</span>}
      </div>
      <div className="suna-settings-runtime-grid">
        <div className="suna-settings-runtime-card">
          <label className="suna-settings-field">
            <div className="suna-settings-field-header">
              <span>Temperature</span>
              <span className="suna-settings-val-chip">{value.temperature.toFixed(1)}</span>
            </div>
            <Input disabled={loading} type="number" min="0" max="2" step="0.1" value={value.temperature} onChange={(event) => void update({ temperature: Number(event.target.value) })} />
          </label>
          <div className="suna-settings-preset-row">
            <button type="button" className={`suna-settings-preset-btn ${value.temperature === 0.1 ? "is-active" : ""}`} onClick={() => void update({ temperature: 0.1 })}>严谨 0.1</button>
            <button type="button" className={`suna-settings-preset-btn ${value.temperature === 0.2 ? "is-active" : ""}`} onClick={() => void update({ temperature: 0.2 })}>默认 0.2</button>
            <button type="button" className={`suna-settings-preset-btn ${value.temperature === 0.5 ? "is-active" : ""}`} onClick={() => void update({ temperature: 0.5 })}>平衡 0.5</button>
            <button type="button" className={`suna-settings-preset-btn ${value.temperature === 0.8 ? "is-active" : ""}`} onClick={() => void update({ temperature: 0.8 })}>发散 0.8</button>
          </div>
          <span className="suna-settings-field-hint">控制模型推理随机性，科研数值计算推荐 0.1~0.2</span>
        </div>

        <div className="suna-settings-runtime-card">
          <label className="suna-settings-field">
            <div className="suna-settings-field-header">
              <span>Max Tokens</span>
              <span className="suna-settings-val-chip">{value.maxTokens}</span>
            </div>
            <Input disabled={loading} type="number" min="256" max="262144" step="256" value={value.maxTokens} onChange={(event) => void update({ maxTokens: Number(event.target.value) })} />
          </label>
          <div className="suna-settings-preset-row">
            <button type="button" className={`suna-settings-preset-btn ${value.maxTokens === 2048 ? "is-active" : ""}`} onClick={() => void update({ maxTokens: 2048 })}>2048</button>
            <button type="button" className={`suna-settings-preset-btn ${value.maxTokens === 4096 ? "is-active" : ""}`} onClick={() => void update({ maxTokens: 4096 })}>4096</button>
            <button type="button" className={`suna-settings-preset-btn ${value.maxTokens === 8192 ? "is-active" : ""}`} onClick={() => void update({ maxTokens: 8192 })}>8192</button>
            <button type="button" className={`suna-settings-preset-btn ${value.maxTokens === 16384 ? "is-active" : ""}`} onClick={() => void update({ maxTokens: 16384 })}>16K</button>
          </div>
          <span className="suna-settings-field-hint">单次响应最大 Token 生成上限</span>
        </div>

        <div className="suna-settings-runtime-card">
          <label className="suna-settings-field">
            <div className="suna-settings-field-header">
              <span>Context Length</span>
              <span className="suna-settings-val-chip">{value.contextLength >= 1024 ? `${Math.round(value.contextLength / 1024)}K` : value.contextLength}</span>
            </div>
            <Input disabled={loading} type="number" min="1024" max="1048576" step="1024" value={value.contextLength} onChange={(event) => void update({ contextLength: Number(event.target.value) })} />
          </label>
          <div className="suna-settings-preset-row">
            <button type="button" className={`suna-settings-preset-btn ${value.contextLength === 16384 ? "is-active" : ""}`} onClick={() => void update({ contextLength: 16384 })}>16K</button>
            <button type="button" className={`suna-settings-preset-btn ${value.contextLength === 32768 ? "is-active" : ""}`} onClick={() => void update({ contextLength: 32768 })}>32K</button>
            <button type="button" className={`suna-settings-preset-btn ${value.contextLength === 65536 ? "is-active" : ""}`} onClick={() => void update({ contextLength: 65536 })}>64K</button>
            <button type="button" className={`suna-settings-preset-btn ${value.contextLength === 131072 ? "is-active" : ""}`} onClick={() => void update({ contextLength: 131072 })}>128K</button>
          </div>
          <span className="suna-settings-field-hint">Agent 跨轮次上下文记忆保留容量</span>
        </div>

        <div className="suna-settings-runtime-card">
          <label className="suna-settings-field">
            <div className="suna-settings-field-header">
              <span>单次模型请求超时（秒）</span>
              <span className="suna-settings-val-chip">{value.timeoutSeconds}s</span>
            </div>
            <Input disabled={loading} type="number" min="5" max="1800" step="5" value={value.timeoutSeconds} onChange={(event) => void update({ timeoutSeconds: Number(event.target.value) })} />
          </label>
          <div className="suna-settings-preset-row">
            <button type="button" className={`suna-settings-preset-btn ${value.timeoutSeconds === 60 ? "is-active" : ""}`} onClick={() => void update({ timeoutSeconds: 60 })}>60s</button>
            <button type="button" className={`suna-settings-preset-btn ${value.timeoutSeconds === 120 ? "is-active" : ""}`} onClick={() => void update({ timeoutSeconds: 120 })}>120s</button>
            <button type="button" className={`suna-settings-preset-btn ${value.timeoutSeconds === 180 ? "is-active" : ""}`} onClick={() => void update({ timeoutSeconds: 180 })}>180s</button>
            <button type="button" className={`suna-settings-preset-btn ${value.timeoutSeconds === 300 ? "is-active" : ""}`} onClick={() => void update({ timeoutSeconds: 300 })}>300s</button>
          </div>
          <span className="suna-settings-field-hint">大模型接口单次 HTTP 等待最长时限</span>
        </div>
      </div>
      <footer className="suna-settings-form-actions"><Button variant="secondary" type="button" onClick={reset} disabled={loading || state === "saving"}><RotateCcw size={15} />恢复运行参数默认值</Button></footer>
    </section>
  );
}
