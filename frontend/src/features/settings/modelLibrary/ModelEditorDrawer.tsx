import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react";
import { Check, PlugZap, X } from "lucide-react";
import ModelCredentialsPanel from "./ModelCredentialsPanel";
import { providerByIdFromCache } from "./useModelProviders";
import type { ProfileModelType, ProviderProfileResponse } from "../../../bridge/desktop";
import { desktop, isDesktopRuntime } from "../../../bridge/desktop";
import SettingsProviderCard from "../SettingsProviderCard";
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
  type SettingsEditor,
} from "../settingsModel";
import type { ProviderCapability, ProviderProfileInput } from "../../../bridge/desktop";
import { useLocale } from "../../../i18n/locale";
import { settingsErrorMessage } from "../settingsError";
import { Button } from "../../../components/ui/button";
import ModelCatalogPicker from "./ModelCatalogPicker";
import OllamaModelDownload from "./OllamaModelDownload";

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
            <p>类型：{MODEL_TYPE_LABELS[modelType] ?? modelType}</p>
          </div>
          <Button variant="ghost" size="icon" type="button" onClick={onClose} aria-label="关闭">
            <X size={18} />
          </Button>
        </header>
        <div className="suna-model-drawer-body">
          {error && <p className="suna-settings-inline-error" role="alert">{error}</p>}
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
                      const slot = slotForModelType(type);
                      setEditor((current) =>
                        current ? editorFor(slot, undefined, undefined) : current,
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
              <label>
                <input
                  type="radio"
                  name="model-source"
                  checked={remoteSource}
                  onChange={() => setRemoteSource(true)}
                />
                远程 API
              </label>
              <label>
                <input
                  type="radio"
                  name="model-source"
                  checked={!remoteSource}
                  onChange={() => setRemoteSource(false)}
                />
                本地服务
              </label>
            </fieldset>
          )}
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
          <ModelCredentialsPanel
            profileId={editor.id}
            kind={editor.kind}
            onPrimarySaved={(name, value) => {
              if (name === "api_key") {
                setEditor((current) => (current ? { ...current, apiKey: value } : current));
              }
            }}
          />
          <label className="suna-settings-field">
            <span>高级配置（JSON，可选）</span>
            <textarea
              className="suna-settings-textarea"
              rows={4}
              value={advancedJson}
              onChange={(event) => setAdvancedJson(event.target.value)}
              placeholder='{"custom_headers":{"X-Custom":"1"},"thinking":"medium"}'
            />
          </label>
          <label className="suna-model-default-toggle">
            <input
              type="checkbox"
              checked={setAsDefault}
              onChange={(event) => setSetAsDefault(event.target.checked)}
            />
            保存后设为该类型的默认模型
          </label>
          <SettingsProviderCard
            editor={{ ...editor, enabled: true }}
            busy={busy}
            testing={testing}
            onChange={setEditor}
            onSubmit={save}
            onTest={test}
            onDelete={() => undefined}
            hideEnabledToggle
            hideDelete
            hideApiKey
            hideCardActions
          />
        </div>
        <footer className="suna-model-drawer-footer">
          <div className="suna-model-drawer-test">
            <Button
              type="button"
              variant="secondary"
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
          <Button type="submit" form={`settings-${slot}-card`} variant="primary" disabled={busy}>
            {busy ? t("saving") : "保存并关闭"}
          </Button>
        </footer>
      </aside>
    </div>
  );
}
