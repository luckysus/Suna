import type { FormEvent } from "react";
import { Bot, Check, Database, KeyRound, Lock, PlugZap, Save, Sparkles, Trash2 } from "lucide-react";
import { useLocale } from "../../i18n/locale";
import {
  type ProviderSlot,
  type SettingsEditor,
  slotTitles,
} from "./settingsModel";
import type { ProviderKind } from "../../bridge/desktop";
import { Checkbox } from "../../components/ui/checkbox";
import { Input } from "../../components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../../components/ui/select";
import { Button } from "../../components/ui/button";

interface SettingsProviderCardProps {
  editor: SettingsEditor;
  busy: boolean;
  testing: boolean;
  onChange: (editor: SettingsEditor) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>, editor: SettingsEditor) => void;
  onTest: (editor: SettingsEditor) => void;
  onDelete: (editor: SettingsEditor) => void;
}

export default function SettingsProviderCard({
  editor,
  busy,
  testing,
  onChange,
  onSubmit,
  onTest,
  onDelete,
}: SettingsProviderCardProps) {
  const { t } = useLocale();
  const update = <K extends keyof SettingsEditor>(key: K, value: SettingsEditor[K]) => {
    onChange({ ...editor, [key]: value } as SettingsEditor);
  };
  const updateKind = (kind: ProviderKind) => {
    const retrieval = editor.slot === "embedding" || editor.slot === "reranker";
    const preset = retrieval
      ? kind === "qwen"
        ? { displayName: "阿里百炼", baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1", modelId: editor.slot === "embedding" ? "qwen3.7-text-embedding" : "qwen3.7-text-rerank" }
        : { displayName: "SiliconFlow", baseUrl: "https://api.siliconflow.cn/v1", modelId: editor.slot === "embedding" ? "BAAI/bge-m3" : "BAAI/bge-reranker-v2-m3" }
      : kind === "deepseek"
      ? { displayName: "DeepSeek", baseUrl: "https://api.deepseek.com", modelId: "deepseek-v4-flash" }
      : kind === "anthropic"
        ? { displayName: "Anthropic", baseUrl: "https://api.anthropic.com/v1", modelId: "claude-sonnet-4" }
        : kind === "qwen"
          ? { displayName: "Qwen", baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1", modelId: "qwen-plus" }
          : kind === "ollama"
            ? { displayName: "Ollama", baseUrl: "http://127.0.0.1:11434", modelId: "qwen3" }
            : { displayName: "OpenAI Compatible", baseUrl: "https://api.openai.com/v1", modelId: "gpt-4o-mini" };
    onChange({ ...editor, kind, ...preset });
  };

  const slotKickers: Record<ProviderSlot, string> = {
    chat: "LLM REASONING",
    embedding: "VECTOR EMBEDDING",
    reranker: "CROSS-ENCODER",
    mineru: "DOCUMENT PARSER",
  };

  return (
    <form className="suna-settings-card" id={`settings-${editor.slot}-card`} onSubmit={(event) => void onSubmit(event, editor)}>
      <div className="suna-settings-card-heading">
        <div className="suna-settings-card-heading-left">
          <span className={`suna-settings-card-icon slot-${editor.slot}`}>
            {editor.slot === "chat" ? <Bot size={17} /> : editor.slot === "embedding" ? <Sparkles size={17} /> : editor.slot === "reranker" ? <Check size={17} /> : <Database size={17} />}
          </span>
          <div>
            <span className="suna-settings-card-kicker">{slotKickers[editor.slot] || "PROVIDER"}</span>
            <h2>{t(slotTitles[editor.slot])}</h2>
          </div>
        </div>
        <div className="suna-settings-card-heading-right">
          <span className={`suna-settings-status ${editor.secretConfigured ? "is-configured" : "is-missing"}`}>
            {editor.secretConfigured ? <Lock size={12} /> : <KeyRound size={12} />}
            {editor.secretConfigured ? t("settingsSecretConfigured") : t("settingsSecretMissing")}
          </span>
          <label className="suna-settings-enabled">
            <Checkbox aria-label={t("settingsEnabled")} checked={editor.enabled} onCheckedChange={(value) => update("enabled", value === true)} />
            {t("settingsEnabled")}
          </label>
        </div>
      </div>

      <div className="suna-settings-fields">
        <div className="suna-settings-fields-row-2">
          {editor.slot !== "mineru" && (
            <div className="suna-settings-field-col">
              <label htmlFor={`settings-${editor.slot}-kind`}>{t("settingsProviderType")}</label>
              <Select value={editor.kind} onValueChange={(value) => updateKind(value as ProviderKind)}>
                <SelectTrigger id={`settings-${editor.slot}-kind`} aria-label={t("settingsProviderType")}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {editor.slot === "chat" ? (
                    <>
                      <SelectItem value="deepseek">{t("providerDeepSeek")}</SelectItem>
                      <SelectItem value="anthropic">{t("providerAnthropic")}</SelectItem>
                      <SelectItem value="qwen">{t("providerQwen")}</SelectItem>
                      <SelectItem value="open_ai_compatible">{t("providerOpenAiCompatible")}</SelectItem>
                      <SelectItem value="ollama">{t("providerOllama")}</SelectItem>
                    </>
                  ) : (
                    <>
                      <SelectItem value="siliconflow">SiliconFlow</SelectItem>
                      <SelectItem value="qwen">阿里百炼 Qwen</SelectItem>
                    </>
                  )}
                </SelectContent>
              </Select>
            </div>
          )}
          <div className="suna-settings-field-col">
            <label htmlFor={`settings-${editor.slot}-name`}>{t("settingsDisplayName")}</label>
            <Input id={`settings-${editor.slot}-name`} value={editor.displayName} onChange={(event) => update("displayName", event.target.value)} required />
          </div>
        </div>

        <div className="suna-settings-field-full">
          <label htmlFor={`settings-${editor.slot}-url`}>{t("settingsBaseUrl")}</label>
          <Input id={`settings-${editor.slot}-url`} className="font-mono text-xs" value={editor.baseUrl} onChange={(event) => update("baseUrl", event.target.value)} required />
        </div>

        {editor.kind !== "mineru" && (
          <div className="suna-settings-field-full">
            <label htmlFor={`settings-${editor.slot}-model`}>{t("settingsModelId")}</label>
            <Input id={`settings-${editor.slot}-model`} className="font-mono text-xs" list={editor.kind === "deepseek" ? "deepseek-models" : undefined} value={editor.modelId} onChange={(event) => update("modelId", event.target.value)} required />
            {editor.kind === "deepseek" && (
              <datalist id="deepseek-models">
                <option value="deepseek-v4-flash" />
                <option value="deepseek-v4-pro" />
              </datalist>
            )}
          </div>
        )}

        <div className="suna-settings-fields-row-2">
          <div className="suna-settings-field-col">
            <label htmlFor={`settings-${editor.slot}-temperature`}>Temperature</label>
            <Input id={`settings-${editor.slot}-temperature`} className="font-mono" type="number" min="0" max="2" step="0.1" value={editor.temperature} onChange={(event) => update("temperature", Number(event.target.value))} />
          </div>
          <div className="suna-settings-field-col">
            <label htmlFor={`settings-${editor.slot}-max-tokens`}>Max Tokens</label>
            <Input id={`settings-${editor.slot}-max-tokens`} className="font-mono" type="number" min="256" max="262144" step="256" value={editor.maxTokens} onChange={(event) => update("maxTokens", Number(event.target.value))} />
          </div>
        </div>

        <div className="suna-settings-field-full">
          <label htmlFor={`settings-${editor.slot}-key`}>{t("settingsApiKey")}</label>
          <Input id={`settings-${editor.slot}-key`} className="font-mono" aria-label={`provider.${editor.slot}.apiKey`} type="password" autoComplete="new-password" value={editor.apiKey} onChange={(event) => update("apiKey", event.target.value)} placeholder={t("settingsApiKeyPlaceholder")} />
          <span className="suna-settings-field-security-note">API Key 将加密写入操作系统本地凭据管理器，不会以明文暴露。</span>
        </div>
      </div>

      <div className="suna-settings-card-actions">
        <Button variant="action" type="submit" disabled={busy}><Save size={16} aria-hidden="true" />{busy ? t("saving") : t("settingsSave")}</Button>
        <Button variant="actionSecondary" type="button" onClick={() => void onTest(editor)} disabled={testing || busy}><PlugZap size={16} aria-hidden="true" />{testing ? t("testing") : t("settingsTest")}</Button>
        {editor.id && <Button size="icon" className="suna-settings-delete" type="button" onClick={() => void onDelete(editor)} disabled={busy} aria-label={`${t("settingsDelete")} ${editor.displayName}`} title={t("settingsDelete")}><Trash2 size={16} aria-hidden="true" /></Button>}
      </div>
    </form>
  );
}

export type { ProviderSlot };
