import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react";
import { Check, PlugZap, X } from "lucide-react";
import ModelCredentialsPanel from "./ModelCredentialsPanel";
import { providerByIdFromCache } from "./useModelProviders";
import type { ProfileModelType, ProviderKind, ProviderProfileResponse } from "../../../bridge/desktop";
import { desktop, isDesktopRuntime } from "../../../bridge/desktop";
import {
  capabilityForModelType,
  inferProfileModelType,
  MODEL_TYPE_LABELS,
  slotForModelType,
} from "./modelLibraryUtils";
import {
  editorFor,
  emitProviderProfilesChanged,
  getSettingValue,
  parseObject,
  providerErrorMessage,
  setSettingValue,
  type ProviderRuntimeMeta,
  type ProviderSlot,
  type SettingsEditor,
} from "../settingsModel";
import type { ProviderCapability, ProviderProfileInput } from "../../../bridge/desktop";
import { useLocale } from "../../../i18n/locale";
import { settingsErrorMessage } from "../settingsError";
import { Button } from "../../../components/ui/button";
import { Input } from "../../../components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../../../components/ui/select";
import ModelCatalogPicker from "./ModelCatalogPicker";
import OllamaModelDownload from "./OllamaModelDownload";

const KIND_LABELS: Record<ProviderKind, string> = {
  deepseek: "DeepSeek",
  anthropic: "Anthropic",
  qwen: "阿里百炼 Qwen",
  open_ai_compatible: "OpenAI Compatible",
  ollama: "Ollama",
  siliconflow: "SiliconFlow",
  mineru: "MinerU",
};

function kindOptionsForSlot(slot: ProviderSlot, current: ProviderKind): ProviderKind[] {
  const base: ProviderKind[] =
    slot === "chat"
      ? ["deepseek", "anthropic", "qwen", "open_ai_compatible", "ollama", "siliconflow"]
      : slot === "embedding"
        ? ["siliconflow", "qwen", "open_ai_compatible", "ollama"]
        : slot === "reranker"
          ? ["siliconflow", "qwen", "open_ai_compatible"]
          : ["mineru"];
  return base.includes(current) ? base : [...base, current];
}

function applyKindPreset(editor: SettingsEditor, kind: ProviderKind): SettingsEditor {
  const retrieval = editor.slot === "embedding" || editor.slot === "reranker";
  const preset = retrieval
    ? kind === "qwen"
      ? {
          displayName: "阿里百炼",
          baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
          modelId: editor.slot === "embedding" ? "qwen3.7-text-embedding" : "qwen3.7-text-rerank",
        }
      : kind === "ollama"
        ? {
            displayName: "Ollama",
            baseUrl: "http://127.0.0.1:11434",
            modelId: editor.slot === "embedding" ? "nomic-embed-text" : editor.modelId,
          }
        : kind === "open_ai_compatible"
          ? {
              displayName: "OpenAI Compatible",
              baseUrl: "https://api.openai.com/v1",
              modelId: editor.slot === "embedding" ? "text-embedding-3-small" : "BAAI/bge-reranker-v2-m3",
            }
          : {
              displayName: "SiliconFlow",
              baseUrl: "https://api.siliconflow.cn/v1",
              modelId: editor.slot === "embedding" ? "BAAI/bge-m3" : "BAAI/bge-reranker-v2-m3",
            }
    : kind === "deepseek"
      ? { displayName: "DeepSeek", baseUrl: "https://api.deepseek.com", modelId: "deepseek-v4-flash" }
      : kind === "anthropic"
        ? { displayName: "Anthropic", baseUrl: "https://api.anthropic.com/v1", modelId: "claude-sonnet-4" }
        : kind === "qwen"
          ? { displayName: "Qwen", baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1", modelId: "qwen-plus" }
          : kind === "ollama"
            ? { displayName: "Ollama", baseUrl: "http://127.0.0.1:11434", modelId: "qwen3" }
            : kind === "siliconflow"
              ? {
                  displayName: "SiliconFlow",
                  baseUrl: "https://api.siliconflow.cn/v1",
                  modelId: "deepseek-ai/DeepSeek-V3",
                }
              : {
                  displayName: "OpenAI Compatible",
                  baseUrl: "https://api.openai.com/v1",
                  modelId: "gpt-4o-mini",
                };
  return { ...editor, kind, ...preset };
}

const CREATE_TYPES: ProfileModelType[] = [
  "chat",
  "embedding",
  "rerank",
  "document_parser",
  "vllm",
  "asr",
];

function parseMetadataObject(raw?: string | null): Record<string, unknown> {
  if (!raw?.trim()) return {};
  try {
    const value = JSON.parse(raw) as unknown;
    return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
  } catch {
    return {};
  }
}

export default function ModelEditorDrawer({
  open,
  profile,
  createType,
  onClose,
  onSaved,
}: {
  open: boolean;
  profile: ProviderProfileResponse | null;
  createType: ProfileModelType;
  onClose: () => void;
  onSaved: () => void;
}) {
  const { t } = useLocale();
  const [editor, setEditor] = useState<SettingsEditor | null>(null);
  const [draftModelType, setDraftModelType] = useState<ProfileModelType>(createType);
  const [catalogProviderId, setCatalogProviderId] = useState<string | null>(null);
  const [remoteSource, setRemoteSource] = useState(true);
  const [advancedJson, setAdvancedJson] = useState("");
  const [busy, setBusy] = useState(false);
  const [testing, setTesting] = useState(false);
  const [setAsDefault, setSetAsDefault] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [testOk, setTestOk] = useState<boolean | null>(null);
  const [vendorExtras, setVendorExtras] = useState<Record<string, string>>({});

  const modelType = profile ? inferProfileModelType(profile) : draftModelType;

  useEffect(() => {
    if (!open) return;
    setDraftModelType(createType);
    const slot = slotForModelType(profile ? inferProfileModelType(profile) : createType);
    void getSettingValue("model.provider.preferences").then((raw) => {
      const meta = parseObject(raw) as Record<string, ProviderRuntimeMeta>;
      const metaFor = profile?.id ? meta[profile.id] : undefined;
      setEditor(editorFor(slot, profile ?? undefined, metaFor));
      setSetAsDefault(!profile || (profile.default_for?.length ?? 0) > 0);
      setError(null);
      const metadata = parseMetadataObject(profile?.metadata_json);
      const providerId =
        typeof metadata.provider_id === "string" ? metadata.provider_id : null;
      setCatalogProviderId(providerId);
      setRemoteSource(profile?.source !== "local");
      const { capabilities: _caps, provider_id: _pid, ...rest } = metadata;
      const knownExtra = new Set(
        (providerId ? providerByIdFromCache(providerId)?.extra_fields : undefined)?.map((field) => field.id) ?? [],
      );
      const vendor: Record<string, string> = {};
      const advanced: Record<string, unknown> = {};
      for (const [key, value] of Object.entries(rest)) {
        if (knownExtra.has(key) && typeof value === "string") vendor[key] = value;
        else advanced[key] = value;
      }
      setVendorExtras(vendor);
      setAdvancedJson(Object.keys(advanced).length > 0 ? JSON.stringify(advanced, null, 2) : "");
      setTestOk(null);
    });
  }, [open, profile, createType]);

  const source = useMemo(() => {
    if (editor?.kind === "ollama") return "local" as const;
    return remoteSource ? ("remote" as const) : ("local" as const);
  }, [editor?.kind, remoteSource]);

  const selectOllamaModel = useCallback((name: string) => {
    setEditor((current) => (current ? { ...current, modelId: name } : current));
  }, []);

  if (!open || !editor) return null;

  const capability = capabilityForModelType(modelType);

  const save = async (event: FormEvent<HTMLFormElement>, current: SettingsEditor) => {
    event.preventDefault();
    if (!isDesktopRuntime()) {
      setError("请在桌面客户端中保存模型配置");
      return;
    }
    if (modelType === "rerank" && current.kind === "ollama") {
      setError("重排模型不支持本地 Ollama，请选择远程 API 服务商");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const credentialName = current.kind === "ollama" ? null : "api_key";
      let metadata: Record<string, unknown> = { ...vendorExtras };
      if (advancedJson.trim()) {
        const extra = JSON.parse(advancedJson) as unknown;
        if (extra && typeof extra === "object" && !Array.isArray(extra)) {
          metadata = { ...(extra as Record<string, unknown>) };
        }
      }
      if (catalogProviderId) {
        metadata.provider_id = catalogProviderId;
      }
      const resolved = await desktop.resolveModelCatalog(
        catalogProviderId,
        current.modelId || "",
        modelType,
      );
      metadata.capabilities = resolved;

      const input: ProviderProfileInput = {
        id: current.id ?? undefined,
        kind: current.kind,
        display_name: current.displayName,
        base_url: current.baseUrl,
        model_id: current.modelId || null,
        credential_name: credentialName,
        enabled: true,
        model_type: modelType,
        source,
        status: "active",
        provider_id: catalogProviderId,
        metadata_json: JSON.stringify(metadata),
      };
      const saved = await desktop.saveProviderProfile(input);
      if (credentialName && current.apiKey.trim()) {
        await desktop.setProviderSecret(saved.id, credentialName, current.apiKey.trim());
      }
      const providerMeta = parseObject(await getSettingValue("model.provider.preferences"));
      providerMeta[saved.id] = { temperature: current.temperature, max_tokens: current.maxTokens };
      await setSettingValue("model.provider.preferences", JSON.stringify({ version: 1, ...providerMeta }));
      if (setAsDefault) {
        await desktop.setDefaultProvider(capability, saved.id);
      }
      if (modelType === "chat") {
        const completed = parseObject(await getSettingValue("onboarding.completed"));
        await setSettingValue(
          "onboarding.completed",
          JSON.stringify({
            ...completed,
            version: typeof completed.version === "number" ? completed.version : 1,
            completed: true,
            llm_profile_id: setAsDefault ? saved.id : completed.llm_profile_id ?? null,
          }),
        );
      }
      emitProviderProfilesChanged();
      onSaved();
      onClose();
    } catch (cause) {
      setError(settingsErrorMessage(cause, t("settingsSaveError")));
    } finally {
      setBusy(false);
    }
  };

  const test = async (current: SettingsEditor) => {
    if (!current.id) {
      setError(t("settingsSaveBeforeTest"));
      return;
    }
    setTesting(true);
    setError(null);
    setTestOk(null);
    try {
      const result = await desktop.testProviderProfile(current.id, capability as ProviderCapability);
      if (!result.ok) throw new Error(providerErrorMessage(result.error_code, (key) => t(key)));
      setTestOk(true);
    } catch (cause) {
      setTestOk(false);
      setError(settingsErrorMessage(cause, t("settingsTestFailed")));
    } finally {
      setTesting(false);
    }
  };

  const slot = slotForModelType(modelType);
  const catalogProvider = catalogProviderId ? providerByIdFromCache(catalogProviderId) : undefined;

  return (
    <div className="suna-model-drawer-overlay" role="presentation" onClick={onClose}>
      <aside
        className="suna-model-drawer"
        role="dialog"
        aria-modal="true"
        aria-label={profile ? "编辑模型" : "添加模型"}
        onClick={(event) => event.stopPropagation()}
      >
        <header className="suna-model-drawer-head">
          <div>
            <span className="suna-settings-kicker">MODEL</span>
            <h3>{profile ? "编辑模型" : "添加模型"}</h3>
            <p className="suna-model-drawer-sub">从目录选择服务商，确认连接参数后保存。</p>
          </div>
          <div className="suna-model-drawer-head-right">
            <span className="suna-model-type-badge">{MODEL_TYPE_LABELS[modelType] ?? modelType}</span>
            <Button variant="ghost" size="icon" type="button" onClick={onClose} aria-label="关闭">
              <X size={18} />
            </Button>
          </div>
        </header>
        <form
          className="suna-model-drawer-form"
          id={`settings-${slot}-card`}
          onSubmit={(event) => void save(event, editor)}
        >
          <div className="suna-model-drawer-body">
            {error && <p className="suna-settings-inline-error" role="alert">{error}</p>}

            <section className="suna-model-drawer-section">
              <header className="suna-model-drawer-section-head">
                <span className="suna-model-drawer-step">1</span>
                <div>
                  <h4>基础设置</h4>
                  <p>先确定能力类型与部署方式</p>
                </div>
              </header>
              {!profile && (
                <fieldset className="suna-model-type-picker">
                  <legend>模型类型</legend>
                  <div className="suna-model-type-options">
                    {CREATE_TYPES.map((type) => (
                      <button
                        key={type}
                        type="button"
                        className={draftModelType === type ? "is-active" : ""}
                        onClick={() => {
                          setDraftModelType(type);
                          const nextSlot = slotForModelType(type);
                          setEditor((current) =>
                            current ? editorFor(nextSlot, undefined, undefined) : current,
                          );
                        }}
                      >
                        {MODEL_TYPE_LABELS[type]}
                      </button>
                    ))}
                  </div>
                </fieldset>
              )}
              {editor.kind !== "ollama" && (
                <fieldset className="suna-model-source-picker">
                  <legend>部署方式</legend>
                  <div className="suna-model-source-options">
                    <label className={remoteSource ? "is-active" : ""}>
                      <input
                        type="radio"
                        name="model-source"
                        checked={remoteSource}
                        onChange={() => setRemoteSource(true)}
                      />
                      远程 API
                    </label>
                    <label className={!remoteSource ? "is-active" : ""}>
                      <input
                        type="radio"
                        name="model-source"
                        checked={!remoteSource}
                        onChange={() => setRemoteSource(false)}
                      />
                      本地服务
                    </label>
                  </div>
                </fieldset>
              )}
            </section>

            <section className="suna-model-drawer-section">
              <header className="suna-model-drawer-section-head">
                <span className="suna-model-drawer-step">2</span>
                <div>
                  <h4>选择服务商与模型</h4>
                  <p>点击目录项会自动填充下方连接参数</p>
                </div>
              </header>
              <ModelCatalogPicker
                modelType={modelType}
                kind={editor.kind}
                modelId={editor.modelId}
                onApply={(patch) => {
                  setCatalogProviderId(patch.providerId);
                  setEditor((current) =>
                    current
                      ? {
                          ...current,
                          kind: patch.kind,
                          displayName: patch.displayName,
                          baseUrl: patch.baseUrl,
                          modelId: patch.modelId,
                        }
                      : current,
                  );
                }}
              />
              {editor.kind === "ollama" && (
                <OllamaModelDownload
                  baseUrl={editor.baseUrl}
                  onSelectModel={selectOllamaModel}
                />
              )}
              {(catalogProvider?.extra_fields?.length ?? 0) > 0 && (
                <div className="suna-model-vendor-extra">
                  {catalogProvider!.extra_fields!.map((field) => (
                    <label key={field.id} className="suna-settings-field">
                      <span>{field.label}{field.required ? " *" : ""}</span>
                      <input
                        value={vendorExtras[field.id] ?? ""}
                        placeholder={field.placeholder ?? ""}
                        onChange={(event) =>
                          setVendorExtras((current) => ({ ...current, [field.id]: event.target.value }))
                        }
                      />
                    </label>
                  ))}
                </div>
              )}
            </section>

            <section className="suna-model-drawer-section">
              <header className="suna-model-drawer-section-head">
                <span className="suna-model-drawer-step">3</span>
                <div>
                  <h4>连接参数</h4>
                  <p>可手动微调显示名、地址与模型 ID</p>
                </div>
              </header>
              <div className="suna-settings-fields suna-model-drawer-fields">
                <div className="suna-settings-fields-row-2">
                  {editor.slot !== "mineru" && (
                    <div className="suna-settings-field-col">
                      <label htmlFor={`settings-${slot}-kind`}>{t("settingsProviderType")}</label>
                      <Select
                        value={editor.kind}
                        onValueChange={(value) =>
                          setEditor((current) =>
                            current ? applyKindPreset(current, value as ProviderKind) : current,
                          )
                        }
                      >
                        <SelectTrigger id={`settings-${slot}-kind`} aria-label={t("settingsProviderType")}>
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          {kindOptionsForSlot(slot, editor.kind).map((kind) => (
                            <SelectItem key={kind} value={kind}>
                              {kind === "deepseek"
                                ? t("providerDeepSeek")
                                : kind === "anthropic"
                                  ? t("providerAnthropic")
                                  : kind === "qwen"
                                    ? t("providerQwen")
                                    : kind === "open_ai_compatible"
                                      ? t("providerOpenAiCompatible")
                                      : kind === "ollama"
                                        ? t("providerOllama")
                                        : KIND_LABELS[kind]}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                  )}
                  <div className="suna-settings-field-col">
                    <label htmlFor={`settings-${slot}-name`}>{t("settingsDisplayName")}</label>
                    <Input
                      id={`settings-${slot}-name`}
                      value={editor.displayName}
                      onChange={(event) =>
                        setEditor((current) =>
                          current ? { ...current, displayName: event.target.value } : current,
                        )
                      }
                      required
                    />
                  </div>
                </div>
                <div className="suna-settings-fields-row-2">
                  <div className="suna-settings-field-col">
                    <label htmlFor={`settings-${slot}-url`}>{t("settingsBaseUrl")}</label>
                    <Input
                      id={`settings-${slot}-url`}
                      className="font-mono text-xs"
                      value={editor.baseUrl}
                      onChange={(event) =>
                        setEditor((current) =>
                          current ? { ...current, baseUrl: event.target.value } : current,
                        )
                      }
                      required
                    />
                  </div>
                  <div className="suna-settings-field-col">
                    <label htmlFor={`settings-${slot}-model`}>{t("settingsModelId")}</label>
                    <Input
                      id={`settings-${slot}-model`}
                      className="font-mono text-xs"
                      value={editor.modelId}
                      onChange={(event) =>
                        setEditor((current) =>
                          current ? { ...current, modelId: event.target.value } : current,
                        )
                      }
                      required={editor.kind !== "mineru"}
                    />
                  </div>
                </div>
                {editor.slot === "chat" && (
                  <div className="suna-settings-fields-row-2">
                    <div className="suna-settings-field-col">
                      <label htmlFor={`settings-${slot}-temperature`}>Temperature</label>
                      <Input
                        id={`settings-${slot}-temperature`}
                        className="font-mono"
                        type="number"
                        min="0"
                        max="2"
                        step="0.1"
                        value={editor.temperature}
                        onChange={(event) =>
                          setEditor((current) =>
                            current ? { ...current, temperature: Number(event.target.value) } : current,
                          )
                        }
                      />
                    </div>
                    <div className="suna-settings-field-col">
                      <label htmlFor={`settings-${slot}-max-tokens`}>Max Tokens</label>
                      <Input
                        id={`settings-${slot}-max-tokens`}
                        className="font-mono"
                        type="number"
                        min="256"
                        max="262144"
                        step="256"
                        value={editor.maxTokens}
                        onChange={(event) =>
                          setEditor((current) =>
                            current ? { ...current, maxTokens: Number(event.target.value) } : current,
                          )
                        }
                      />
                    </div>
                  </div>
                )}
              </div>
            </section>

            <section className="suna-model-drawer-section">
              <header className="suna-model-drawer-section-head">
                <span className="suna-model-drawer-step">4</span>
                <div>
                  <h4>凭据与选项</h4>
                  <p>API Key 保存在系统凭据库；高级项可选</p>
                </div>
              </header>
              <ModelCredentialsPanel
                profileId={editor.id}
                kind={editor.kind}
                onPrimarySaved={(name, value) => {
                  if (name === "api_key") {
                    setEditor((current) => (current ? { ...current, apiKey: value } : current));
                  }
                }}
              />
              <details className="suna-model-advanced">
                <summary>高级配置（JSON，可选）</summary>
                <label className="suna-settings-field">
                  <span className="sr-only">高级配置 JSON</span>
                  <textarea
                    className="suna-settings-textarea"
                    rows={4}
                    value={advancedJson}
                    onChange={(event) => setAdvancedJson(event.target.value)}
                    placeholder='{"custom_headers":{"X-Custom":"1"},"thinking":"medium"}'
                  />
                </label>
              </details>
              <label className="suna-model-default-toggle">
                <input
                  type="checkbox"
                  checked={setAsDefault}
                  onChange={(event) => setSetAsDefault(event.target.checked)}
                />
                保存后设为该类型的默认模型
              </label>
            </section>
          </div>
          <footer className="suna-model-drawer-footer">
            <div className="suna-model-drawer-test">
              <Button
                type="button"
                variant="actionSecondary"
                disabled={testing || busy || !editor.id}
                onClick={() => void test(editor)}
              >
                <PlugZap size={16} />
                {testing ? t("testing") : "测试连接"}
              </Button>
              {testOk === true && <span className="suna-model-test-ok" role="status"><Check size={16} /> 成功</span>}
              {testOk === false && <span className="suna-model-test-fail" role="status">失败</span>}
              {!editor.id && <span className="suna-settings-field-hint">{t("settingsSaveBeforeTest")}</span>}
            </div>
            <Button type="submit" variant="action" disabled={busy}>
              {busy ? t("saving") : "保存并关闭"}
            </Button>
          </footer>
        </form>
      </aside>
    </div>
  );
}
