import { useCallback, useEffect, useState } from "react";
import { Eye, EyeOff, Trash2 } from "lucide-react";
import type { CredentialFieldView } from "../../../bridge/desktop";
import { desktop, isDesktopRuntime } from "../../../bridge/desktop";
import { Button } from "../../../components/ui/button";
import { Input } from "../../../components/ui/input";
import { settingsErrorMessage } from "../settingsError";

export default function ModelCredentialsPanel({
  profileId,
  kind,
  onPrimarySaved,
}: {
  profileId: string | null;
  kind: string;
  onPrimarySaved?: (name: string, value: string) => void;
}) {
  const [fields, setFields] = useState<CredentialFieldView[]>([]);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [visible, setVisible] = useState<Record<string, boolean>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!profileId || !isDesktopRuntime() || kind === "ollama") {
      setFields([]);
      return;
    }
    try {
      const list = await desktop.listProviderCredentialFields(profileId);
      setFields(list);
    } catch {
      setFields([{ name: "api_key", label: "API Key", secret: true, primary: true, configured: false }]);
    }
  }, [profileId, kind]);

  useEffect(() => {
    void load();
  }, [load]);

  const saveField = async (field: CredentialFieldView) => {
    const value = drafts[field.name]?.trim();
    if (!value || !profileId) return;
    setBusy(field.name);
    setError(null);
    try {
      if (field.primary || field.name === "api_key") {
        onPrimarySaved?.(field.name, value);
        await desktop.setProviderSecret(profileId, field.name, value);
      } else {
        await desktop.setProviderAuxiliarySecret(profileId, field.name, value);
      }
      setDrafts((current) => ({ ...current, [field.name]: "" }));
      await load();
    } catch (cause) {
      setError(settingsErrorMessage(cause, "保存凭据失败"));
    } finally {
      setBusy(null);
    }
  };

  const deleteField = async (field: CredentialFieldView) => {
    if (!profileId) return;
    setBusy(field.name);
    setError(null);
    try {
      if (field.primary || field.name === "api_key") {
        await desktop.deleteProviderSecret(profileId, field.name);
      } else {
        await desktop.deleteProviderAuxiliarySecret(profileId, field.name);
      }
      await load();
    } catch (cause) {
      setError(settingsErrorMessage(cause, "删除凭据失败"));
    } finally {
      setBusy(null);
    }
  };

  if (kind === "ollama") {
    return null;
  }

  if (!profileId) {
    return (
      <section className="suna-model-credentials" aria-label="模型凭据">
        <h4>凭据</h4>
        <p className="suna-settings-field-hint">保存模型后可按字段单独配置 API Key 等密钥。</p>
      </section>
    );
  }

  if (fields.length === 0) {
    return null;
  }

  return (
    <section className="suna-model-credentials" aria-label="模型凭据">
      <h4>凭据</h4>
      <p className="suna-settings-field-hint">按字段单独配置与删除，列表中不展示明文。</p>
      {error && <p className="suna-settings-inline-error" role="alert">{error}</p>}
      <ul className="suna-model-credentials-list">
        {fields.map((field) => (
          <li key={field.name}>
            <div className="suna-model-credentials-row">
              <label>
                <span>{field.label}</span>
                <span className={`suna-model-credential-status ${field.configured ? "is-set" : ""}`}>
                  {field.configured ? "已配置" : "未配置"}
                </span>
              </label>
              {field.secret && (
                <div className="suna-model-credentials-input">
                  <Input
                    type={visible[field.name] ? "text" : "password"}
                    autoComplete="new-password"
                    placeholder={field.configured ? "输入新值以替换" : "输入密钥"}
                    value={drafts[field.name] ?? ""}
                    onChange={(event) =>
                      setDrafts((current) => ({ ...current, [field.name]: event.target.value }))
                    }
                  />
                  <button
                    type="button"
                    className="suna-settings-pwd-toggle"
                    onClick={() => setVisible((current) => ({ ...current, [field.name]: !current[field.name] }))}
                    aria-label={visible[field.name] ? "隐藏" : "显示"}
                  >
                    {visible[field.name] ? <EyeOff size={14} /> : <Eye size={14} />}
                  </button>
                </div>
              )}
              <div className="suna-model-credentials-actions">
                <Button
                  type="button"
                  variant="secondary"
                  disabled={!drafts[field.name]?.trim() || busy === field.name}
                  onClick={() => void saveField(field)}
                >
                  保存字段
                </Button>
                {field.configured && (
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    disabled={busy === field.name}
                    aria-label={`删除 ${field.label}`}
                    onClick={() => void deleteField(field)}
                  >
                    <Trash2 size={14} />
                  </Button>
                )}
              </div>
            </div>
          </li>
        ))}
      </ul>
    </section>
  );
}
