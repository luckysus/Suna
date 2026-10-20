import { useEffect, useMemo, useState } from "react";
import { FlaskConical, Upload, X } from "lucide-react";
import type { ProfileModelType, ProviderDebugInput, ProviderProfileResponse } from "../../../bridge/desktop";
import { desktop, isDesktopRuntime } from "../../../bridge/desktop";
import { inferProfileModelType } from "./modelLibraryUtils";
import { settingsErrorMessage } from "../settingsError";
import { Button } from "../../../components/ui/button";
import { Input } from "../../../components/ui/input";

function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result;
      if (typeof result !== "string") {
        reject(new Error("无法读取文件"));
        return;
      }
      const comma = result.indexOf(",");
      resolve(comma >= 0 ? result.slice(comma + 1) : result);
    };
    reader.onerror = () => reject(reader.error ?? new Error("无法读取文件"));
    reader.readAsDataURL(file);
  });
}

export default function ModelDebugDrawer({
  open,
  profiles,
  onClose,
}: {
  open: boolean;
  profiles: ProviderProfileResponse[];
  onClose: () => void;
}) {
  const [profileId, setProfileId] = useState("");
  const [input, setInput] = useState("");
  const [documents, setDocuments] = useState("");
  const [file, setFile] = useState<File | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [resultJson, setResultJson] = useState<string | null>(null);
  const [similarity, setSimilarity] = useState(false);

  const selected = profiles.find((profile) => profile.id === profileId) ?? profiles[0];
  const modelType: ProfileModelType = selected ? inferProfileModelType(selected) : "chat";

  useEffect(() => {
    if (!open) return;
    setProfileId(profiles[0]?.id ?? "");
    setError(null);
    setResultJson(null);
  }, [open, profiles]);

  const needsFile = modelType === "vllm" || modelType === "asr";
  const needsDocuments = modelType === "rerank";
  const needsEmbeddingLines = modelType === "embedding" && similarity;

  const filteredProfiles = useMemo(
    () => profiles.filter((profile) => profile.enabled),
    [profiles],
  );

  const run = async () => {
    if (!selected || !isDesktopRuntime()) return;
    setBusy(true);
    setError(null);
    setResultJson(null);
    try {
      const payload: ProviderDebugInput = {
        profile_id: selected.id,
        input: input.trim() || null,
        documents: documents
          .split("\n")
          .map((line) => line.trim())
          .filter(Boolean),
        options: { temperature: 0.2, max_tokens: 512, similarity: modelType === "embedding" ? similarity : undefined },
        file_name: file?.name ?? null,
        file_mime: file?.type || null,
        file_base64: file ? await fileToBase64(file) : null,
      };
      const result = await desktop.debugProviderProfile(payload);
      setResultJson(JSON.stringify(result, null, 2));
      if (!result.ok && result.error) {
        setError(result.error);
      }
    } catch (cause) {
      setError(settingsErrorMessage(cause, "模型调试失败"));
    } finally {
      setBusy(false);
    }
  };

  if (!open) return null;

  return (
    <div className="suna-model-drawer-overlay" role="presentation" onClick={onClose}>
      <aside
        className="suna-model-drawer suna-model-debug-drawer"
        role="dialog"
        aria-modal="true"
        aria-label="模型测试"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="suna-model-drawer-head">
          <div>
            <span className="suna-settings-kicker">DEBUG</span>
            <h3><FlaskConical size={18} /> 模型测试</h3>
            <p>按模型类型发送调试请求，支持文本、文档列表与文件上传。</p>
          </div>
          <Button variant="ghost" size="icon" type="button" onClick={onClose} aria-label="关闭">
            <X size={18} />
          </Button>
        </header>
        <div className="suna-model-drawer-body">
          <label className="suna-settings-field">
            <span>选择模型</span>
            <select value={profileId} onChange={(event) => setProfileId(event.target.value)}>
              {filteredProfiles.map((profile) => (
                <option key={profile.id} value={profile.id}>
                  {profile.display_name} ({inferProfileModelType(profile)})
                </option>
              ))}
            </select>
          </label>
          {modelType === "embedding" && (
            <label className="suna-model-default-toggle">
              <input type="checkbox" checked={similarity} onChange={(event) => setSimilarity(event.target.checked)} />
              相似度模式（两段文本，换行分隔）
            </label>
          )}
          <label className="suna-settings-field">
            <span>
              {needsDocuments ? "查询 / 主文本" : needsEmbeddingLines ? "两段文本（换行分隔）" : "输入文本"}
            </span>
            <textarea value={input} onChange={(event) => setInput(event.target.value)} rows={4} />
          </label>
          {needsDocuments && (
            <label className="suna-settings-field">
              <span>候选文档（每行一条）</span>
              <textarea value={documents} onChange={(event) => setDocuments(event.target.value)} rows={5} />
            </label>
          )}
          {(needsFile || modelType === "chat") && (
            <label className="suna-settings-field">
              <span>{needsFile ? "上传文件（必填）" : "可选附件"}</span>
              <input
                type="file"
                accept={modelType === "asr" ? "audio/*" : modelType === "vllm" ? "image/*" : "*/*"}
                onChange={(event) => setFile(event.target.files?.[0] ?? null)}
              />
              {file && <span className="suna-settings-field-hint">{file.name}</span>}
            </label>
          )}
          {error && <p className="suna-settings-inline-error" role="alert">{error}</p>}
          <div className="suna-settings-form-actions">
            <Button type="button" variant="primary" disabled={busy || !selected} onClick={() => void run()}>
              <Upload size={16} />
              {busy ? "测试中…" : "运行测试"}
            </Button>
          </div>
          {resultJson && (
            <pre className="suna-model-debug-output" aria-label="调试输出">{resultJson}</pre>
          )}
        </div>
      </aside>
    </div>
  );
}
