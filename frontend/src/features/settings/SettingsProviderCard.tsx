import type { FormEvent } from "react";
import { PlugZap, Save, Trash2 } from "lucide-react";
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

  return (
    <form className="suna-settings-card" onSubmit={(event) => void onSubmit(event, editor)}>
      <div className="suna-settings-card-heading"><div><span className="suna-settings-card-icon"><PlugZap size={17} aria-hidden="true" /></span><h2>{t(slotTitles[editor.slot])}</h2></div><span className={`suna-settings-status ${editor.secretConfigured ? "is-configured" : "is-missing"}`}>{editor.secretConfigured ? t("settingsSecretConfigured") : t("settingsSecretMissing")}</span></div>
      <div className="suna-settings-fields">
        {editor.slot !== "mineru" && (
          <>
            <label htmlFor={`settings-${editor.slot}-kind`}>{t("settingsProviderType")}</label>
            <Select value={editor.kind} onValueChange={(value) => updateKind(value as ProviderKind)}>
              <SelectTrigger id={`settings-${editor.slot}-kind`} aria-label={t("settingsProviderType")}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {editor.slot === "chat" ? <>
                <SelectItem value="deepseek">{t("providerDeepSeek")}</SelectItem>
                <SelectItem value="anthropic">{t("providerAnthropic")}</SelectItem>
                <SelectItem value="qwen">{t("providerQwen")}</SelectItem>
                <SelectItem value="open_ai_compatible">{t("providerOpenAiCompatible")}</SelectItem>
                <SelectItem value="ollama">{t("providerOllama")}</SelectItem>
                </> : <>
                  <SelectItem value="siliconflow">SiliconFlow</SelectItem>
                  <SelectItem value="qwen">阿里百炼 Qwen</SelectItem>
                </>}
              </SelectContent>
            </Select>
          </>
        )}
        <label htmlFor={`settings-${editor.slot}-name`}>{t("settingsDisplayName")}</label>
        <Input id={`settings-${editor.slot}-name`} value={editor.displayName} onChange={(event) => update("displayName", event.target.value)} required />
        <label htmlFor={`settings-${editor.slot}-url`}>{t("settingsBaseUrl")}</label>
        <Input id={`settings-${editor.slot}-url`} value={editor.baseUrl} onChange={(event) => update("baseUrl", event.target.value)} required />
        {editor.kind !== "mineru" && (
          <>
            <label htmlFor={`settings-${editor.slot}-model`}>{t("settingsModelId")}</label>
            <Input id={`settings-${editor.slot}-model`} list={editor.kind === "deepseek" ? "deepseek-models" : undefined} value={editor.modelId} onChange={(event) => update("modelId", event.target.value)} required />
            {editor.kind === "deepseek" && (
              <datalist id="deepseek-models">
                <option value="deepseek-v4-flash" />
                <option value="deepseek-v4-pro" />
              </datalist>
            )}
          </>
        )}
        <label htmlFor={`settings-${editor.slot}-temperature`}>Temperature</label>
        <Input id={`settings-${editor.slot}-temperature`} type="number" min="0" max="2" step="0.1" value={editor.temperature} onChange={(event) => update("temperature", Number(event.target.value))} />
        <label htmlFor={`settings-${editor.slot}-max-tokens`}>Max Tokens</label>
        <Input id={`settings-${editor.slot}-max-tokens`} type="number" min="256" max="262144" step="256" value={editor.maxTokens} onChange={(event) => update("maxTokens", Number(event.target.value))} />
        <label htmlFor={`settings-${editor.slot}-key`}>{t("settingsApiKey")}</label>
        <Input id={`settings-${editor.slot}-key`} aria-label={`provider.${editor.slot}.apiKey`} type="password" autoComplete="new-password" value={editor.apiKey} onChange={(event) => update("apiKey", event.target.value)} placeholder={t("settingsApiKeyPlaceholder")} />
      </div>
      <label className="suna-settings-enabled"><Checkbox aria-label={t("settingsEnabled")} checked={editor.enabled} onCheckedChange={(value) => update("enabled", value === true)} />{t("settingsEnabled")}</label>
      <div className="suna-settings-card-actions">
        <Button variant="action" type="submit" disabled={busy}><Save size={16} aria-hidden="true" />{busy ? t("saving") : t("settingsSave")}</Button>
        <Button variant="actionSecondary" type="button" onClick={() => void onTest(editor)} disabled={testing || busy}><PlugZap size={16} aria-hidden="true" />{testing ? t("testing") : t("settingsTest")}</Button>
        {editor.id && <Button size="icon" className="suna-settings-delete" type="button" onClick={() => void onDelete(editor)} disabled={busy} aria-label={`${t("settingsDelete")} ${editor.displayName}`} title={t("settingsDelete")}><Trash2 size={16} aria-hidden="true" /></Button>}
      </div>
    </form>
  );
}

export type { ProviderSlot };
