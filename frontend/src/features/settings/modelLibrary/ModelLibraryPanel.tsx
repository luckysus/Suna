import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { LoaderCircle } from "lucide-react";
import { Bot, Check, Cloud, Database, FlaskConical, Image, Mic, MoreHorizontal, Plus, Search, Sparkles, Trash2 } from "lucide-react";
import { useLocale } from "../../../i18n/locale";
import type { RetrievalPlan } from "../settingsModel";
import { RadioGroup, RadioGroupItem } from "../../../components/ui/radio-group";
import type { ProfileModelType, ProviderProfileResponse } from "../../../bridge/desktop";
import { desktop, isDesktopRuntime } from "../../../bridge/desktop";
import ModelEditorDrawer from "./ModelEditorDrawer";
import ModelDebugDrawer from "./ModelDebugDrawer";
import ModelOllamaSettings from "./ModelOllamaSettings";
import { modelLibraryFocusModelEvent } from "./modelLibraryEvents";
import ModelCardMenu, { menuIcons, ModelCardMenuButton } from "./ModelCardMenu";
import { useOllamaDownloadComplete } from "./useOllamaDownloadComplete";
import ModelInUseDialog from "./ModelInUseDialog";
import { parseModelUsageFromError } from "./modelUsage";
import { providerLabel, useModelProviders } from "./useModelProviders";
import {
  capabilityForModelType,
  copyDisplayName,
  inferProfileModelType,
  matchesModelQuery,
  MODEL_TYPE_LABELS,
  type ModelLibraryFilter,
} from "./modelLibraryUtils";
import { emitProviderProfilesChanged, providerProfilesChangedEvent } from "../settingsModel";
import { settingsErrorMessage } from "../settingsError";
import { Button } from "../../../components/ui/button";
import { Input } from "../../../components/ui/input";

const TABS: { id: ModelLibraryFilter; label: string; icon: typeof Bot }[] = [
  { id: "all", label: "全部", icon: Sparkles },
  { id: "chat", label: "对话", icon: Bot },
  { id: "embedding", label: "嵌入", icon: Sparkles },
  { id: "rerank", label: "重排", icon: Check },
  { id: "vllm", label: "视觉", icon: Image },
  { id: "asr", label: "语音", icon: Mic },
  { id: "document_parser", label: "解析", icon: Database },
];

function typeIcon(type: ProfileModelType) {
  if (type === "chat") return Bot;
  if (type === "embedding") return Sparkles;
  if (type === "rerank") return Check;
  if (type === "vllm") return Image;
  if (type === "asr") return Mic;
  return Database;
}

export default function ModelLibraryPanel({
  excludeTypes = [],
  plan,
  onPlanChange,
}: {
  excludeTypes?: ProfileModelType[];
  plan?: RetrievalPlan;
  onPlanChange?: (plan: RetrievalPlan) => void;
}) {
  const { t } = useLocale();
  const [profiles, setProfiles] = useState<ProviderProfileResponse[]>([]);
  const [loading, setLoading] = useState(true);
  const [query, setQuery] = useState("");
  const [activeTab, setActiveTab] = useState<ModelLibraryFilter>("all");
  const [planOpen, setPlanOpen] = useState(false);
  const [ollamaOpen, setOllamaOpen] = useState(false);
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const menuAnchorRef = useRef<HTMLButtonElement>(null);
  useOllamaDownloadComplete();
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [editing, setEditing] = useState<ProviderProfileResponse | null>(null);
  const [createType, setCreateType] = useState<ProfileModelType>("chat");
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [debugOpen, setDebugOpen] = useState(false);
  const [usageDetails, setUsageDetails] = useState<import("../../../bridge/desktop").ModelUsageDetails | null>(null);
  const { providerById } = useModelProviders("all");

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    if (!isDesktopRuntime()) {
      setProfiles([]);
      setLoading(false);
      return;
    }
    try {
      const list = await desktop.listProviderProfiles();
      setProfiles(
        list.map((profile) => ({
          ...profile,
          model_type: profile.model_type ?? inferProfileModelType(profile),
          default_for: profile.default_for ?? [],
        })),
      );
    } catch (cause) {
      setError(settingsErrorMessage(cause, "无法加载模型列表"));
      setProfiles([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
    const onChanged = () => void load();
    window.addEventListener(providerProfilesChangedEvent, onChanged);
    return () => window.removeEventListener(providerProfilesChangedEvent, onChanged);
  }, [load]);

  useEffect(() => {
    const onFocus = (event: Event) => {
      const modelId = (event as CustomEvent<{ modelId?: string }>).detail?.modelId;
      if (!modelId) return;
      const profile = profiles.find((entry) => entry.id === modelId);
      if (profile) {
        setEditing(profile);
        setDrawerOpen(true);
      }
    };
    window.addEventListener(modelLibraryFocusModelEvent, onFocus);
    return () => window.removeEventListener(modelLibraryFocusModelEvent, onFocus);
  }, [profiles]);

  useEffect(() => {
    const onPointerDown = (event: MouseEvent) => {
      if (!panelRef.current?.contains(event.target as Node)) {
        setOpenMenuId(null);
      }
    };
    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, []);

  useEffect(() => {
    const hash = window.location.hash.match(/model=([0-9a-f-]+)/i);
    if (hash?.[1]) {
      window.dispatchEvent(new CustomEvent(modelLibraryFocusModelEvent, { detail: { modelId: hash[1] } }));
    }
  }, []);

  const visibleTabs = TABS.filter((tab) => tab.id === "all" || !excludeTypes.includes(tab.id as ProfileModelType));

  const filtered = useMemo(() => {
    return profiles
      .filter((profile) => !excludeTypes.includes(inferProfileModelType(profile)))
      .filter((profile) => activeTab === "all" || inferProfileModelType(profile) === activeTab)
      .filter((profile) => matchesModelQuery(profile, query));
  }, [profiles, activeTab, query, excludeTypes]);

  const counts = useMemo(() => {
    const base = profiles.filter((p) => !excludeTypes.includes(inferProfileModelType(p)));
    const tally: Record<ModelLibraryFilter, number> = {
      all: base.length,
      chat: 0,
      embedding: 0,
      rerank: 0,
      document_parser: 0,
      vllm: 0,
      asr: 0,
    };
    for (const profile of base) {
      tally[inferProfileModelType(profile)] += 1;
    }
    return tally;
  }, [profiles, excludeTypes]);

  const openCreate = (type: ProfileModelType) => {
    setEditing(null);
    setCreateType(type);
    setDrawerOpen(true);
  };

  const setDefault = async (profile: ProviderProfileResponse) => {
    const type = inferProfileModelType(profile);
    setBusyId(profile.id);
    try {
      await desktop.setDefaultProvider(capabilityForModelType(type), profile.id);
      emitProviderProfilesChanged();
      await load();
    } catch (cause) {
      setError(settingsErrorMessage(cause, "设置默认模型失败"));
    } finally {
      setBusyId(null);
    }
  };

  const copyModel = async (profile: ProviderProfileResponse) => {
    setBusyId(profile.id);
    try {
      const copied = await desktop.copyProviderProfile(profile.id, copyDisplayName(profile.display_name));
      if (
        copied.kind === "ollama"
        && copied.model_id?.trim()
        && (copied.source === "local" || profile.source === "local")
      ) {
        await desktop.saveProviderProfile({
          id: copied.id,
          kind: copied.kind,
          display_name: copied.display_name,
          base_url: copied.base_url,
          model_id: copied.model_id,
          enabled: copied.enabled,
          model_type: inferProfileModelType(copied),
          source: "local",
          status: "downloading",
          metadata_json: copied.metadata_json,
        });
        void desktop.downloadOllamaModel(copied.model_id!, copied.base_url).catch(() => undefined);
      }
      emitProviderProfilesChanged();
      await load();
    } catch (cause) {
      setError(settingsErrorMessage(cause, "复制模型失败"));
    } finally {
      setBusyId(null);
    }
  };

  const deleteModel = async (profile: ProviderProfileResponse) => {
    setBusyId(profile.id);
    setDeleteConfirmId(null);
    try {
      await desktop.deleteProviderProfile(profile.id);
      emitProviderProfilesChanged();
      await load();
    } catch (cause) {
      const usage = parseModelUsageFromError(cause);
      if (usage) {
        setUsageDetails(usage);
        setError(null);
      } else {
        setError(settingsErrorMessage(cause, "删除模型失败"));
      }
    } finally {
      setBusyId(null);
    }
  };

  const disableModel = async (profile: ProviderProfileResponse) => {
    setBusyId(profile.id);
    try {
      await desktop.saveProviderProfile({
        id: profile.id,
        kind: profile.kind,
        display_name: profile.display_name,
        base_url: profile.base_url,
        model_id: profile.model_id,
        enabled: false,
        model_type: inferProfileModelType(profile),
      });
      emitProviderProfilesChanged();
      await load();
    } catch (cause) {
      setError(settingsErrorMessage(cause, "停用模型失败"));
    } finally {
      setBusyId(null);
    }
  };

  const defaultCreateType =
    activeTab !== "all" && !excludeTypes.includes(activeTab as ProfileModelType)
      ? (activeTab as ProfileModelType)
      : excludeTypes.includes("chat")
        ? "embedding"
        : "chat";

  return (
    <div className="suna-model-library" ref={panelRef}>
      <div className="suna-model-library-toolbar">
        <div className="suna-model-library-tabs" role="tablist" aria-label="模型类型">
          {visibleTabs.map((tab) => {
            const Icon = tab.icon;
            return (
              <button
                key={tab.id}
                type="button"
                role="tab"
                aria-selected={activeTab === tab.id}
                className={`suna-model-library-tab ${activeTab === tab.id ? "is-active" : ""}`}
                onClick={() => {
                  setActiveTab(tab.id);
                  setPlanOpen(false);
                  setOllamaOpen(false);
                }}
              >
                <Icon size={15} />
                <span>{tab.label}</span>
                <span className="suna-model-library-tab-count">{counts[tab.id]}</span>
              </button>
            );
          })}
          {plan && onPlanChange && (
            <button
              type="button"
              className={`suna-model-library-tab ${planOpen ? "is-active" : ""}`}
              onClick={() => {
                setPlanOpen(true);
                setOllamaOpen(false);
              }}
            >
              <Cloud size={15} />
              <span>云端专线</span>
            </button>
          )}
          <button
            type="button"
            className={`suna-model-library-tab ${ollamaOpen ? "is-active" : ""}`}
            onClick={() => {
              setOllamaOpen(true);
              setPlanOpen(false);
            }}
          >
            <Bot size={15} />
            <span>Ollama</span>
          </button>
        </div>
        <div className="suna-model-library-actions">
          <div className="suna-model-library-search">
            <Search size={16} />
            <Input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="搜索模型名称、ID 或服务商"
              aria-label="搜索模型"
            />
          </div>
          <Button type="button" variant="secondary" onClick={() => setDebugOpen(true)}>
            <FlaskConical size={16} />
            模型测试
          </Button>
          <Button type="button" variant="primary" onClick={() => openCreate(defaultCreateType)}>
            <Plus size={16} />
            添加模型
          </Button>
        </div>
      </div>

      {error && <p className="suna-settings-inline-error" role="alert">{error}</p>}

      {planOpen && plan && onPlanChange ? (
        <section className="suna-settings-plan" aria-labelledby="settings-plan-heading">
          <h2 id="settings-plan-heading">{t("settingsPlanTitle")}</h2>
          <fieldset className="suna-settings-plan-options">
            <legend>{t("settingsPlanLabel")}</legend>
            <RadioGroup value={plan} onValueChange={(value) => onPlanChange(value as RetrievalPlan)} aria-label={t("settingsPlanLabel")}>
              <label className={`suna-settings-plan-pill ${plan === "free" ? "is-selected" : ""}`}>
                <RadioGroupItem value="free" aria-label={t("settingsPlanFree")} />
                <span>{t("freePlan")}</span>
              </label>
              <label className={`suna-settings-plan-pill ${plan === "pro" ? "is-selected" : ""}`}>
                <RadioGroupItem value="pro" aria-label={t("settingsPlanPro")} />
                <span>{t("proPlan")}</span>
              </label>
            </RadioGroup>
          </fieldset>
        </section>
      ) : ollamaOpen ? (
        <ModelOllamaSettings />
      ) : loading ? (
        <p className="suna-settings-loading" role="status"><LoaderCircle size={18} className="suna-spin" /> 正在加载模型…</p>
      ) : (
        <div className="suna-model-library-grid">
          {filtered.map((profile) => {
            const type = inferProfileModelType(profile);
            const Icon = typeIcon(type);
            const isDefault = (profile.default_for?.length ?? 0) > 0;
            const vendor =
              profile.kind === "ollama"
                ? "Ollama"
                : providerLabel(providerById(profile.kind === "siliconflow" ? "siliconflow" : profile.kind === "qwen" ? "qwen" : profile.kind === "deepseek" ? "deepseek" : profile.kind === "mineru" ? "mineru" : "openai_compatible"));
            const menuOpen = openMenuId === profile.id;
            const capabilities = profile.capabilities as Record<string, unknown> | undefined;
            const contextWindow = typeof capabilities?.context_window === "number" ? capabilities.context_window : null;
            const dimension = typeof capabilities?.dimension === "number" ? capabilities.dimension : null;
            const vision = capabilities?.supports_vision === true;
            return (
              <article
                key={profile.id}
                className={`suna-model-library-card ${!profile.enabled ? "is-disabled" : ""}`}
                role="button"
                tabIndex={0}
                onClick={() => {
                  setEditing(profile);
                  setDrawerOpen(true);
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    setEditing(profile);
                    setDrawerOpen(true);
                  }
                }}
              >
                <div className={`suna-model-library-card-badge slot-${type}`}>
                  <Icon size={16} />
                </div>
                <div className="suna-model-library-card-body">
                  <div className="suna-model-library-card-head">
                    <h3 title={profile.display_name}>{profile.display_name}</h3>
                    <div className="suna-model-library-card-menu" onClick={(e) => e.stopPropagation()}>
                      <button
                        ref={menuOpen ? menuAnchorRef : undefined}
                        type="button"
                        className="suna-model-card-more"
                        aria-label="更多操作"
                        aria-expanded={menuOpen}
                        onClick={() => setOpenMenuId(menuOpen ? null : profile.id)}
                      >
                        <MoreHorizontal size={16} />
                      </button>
                      <ModelCardMenu open={menuOpen} anchorRef={menuAnchorRef} onClose={() => setOpenMenuId(null)}>
                        <ModelCardMenuButton
                          icon={menuIcons.edit}
                          label="编辑"
                          onClick={() => {
                            setEditing(profile);
                            setDrawerOpen(true);
                            setOpenMenuId(null);
                          }}
                        />
                        <ModelCardMenuButton label="设为默认" onClick={() => void setDefault(profile)} disabled={busyId === profile.id} />
                        <ModelCardMenuButton icon={menuIcons.copy} label="复制" onClick={() => void copyModel(profile)} disabled={busyId === profile.id} />
                        {profile.enabled && (
                          <ModelCardMenuButton label="停用" onClick={() => void disableModel(profile)} disabled={busyId === profile.id} />
                        )}
                      </ModelCardMenu>
                      {deleteConfirmId === profile.id ? (
                        <div className="suna-model-delete-confirm">
                          <button type="button" className="is-danger" onClick={() => void deleteModel(profile)}>确认</button>
                          <button type="button" onClick={() => setDeleteConfirmId(null)}>取消</button>
                        </div>
                      ) : (
                        <button
                          type="button"
                          className="suna-model-card-delete"
                          aria-label="删除模型"
                          onClick={() => setDeleteConfirmId(profile.id)}
                          disabled={busyId === profile.id}
                        >
                          <Trash2 size={16} />
                        </button>
                      )}
                    </div>
                  </div>
                  <p className="suna-model-library-card-model">{profile.model_id || "未填写模型 ID"}</p>
                  <div className="suna-model-library-card-meta">
                    <span>{vendor}</span>
                    <span>{MODEL_TYPE_LABELS[type]}</span>
                    <span>{profile.secret_configured ? "凭据已配置" : "凭据缺失"}</span>
                    {isDefault && <span className="suna-model-tag-default">默认</span>}
                    {profile.status === "downloading" && <span className="suna-model-tag-muted">下载中</span>}
                    {profile.status === "download_failed" && <span className="suna-model-tag-muted">下载失败</span>}
                    {profile.source === "local" && <span className="suna-model-tag-muted">本地</span>}
                    {contextWindow && <span>{Math.round(contextWindow / 1024)}K 上下文</span>}
                    {dimension && <span>{dimension} 维</span>}
                    {vision && <span>Vision</span>}
                    {!profile.enabled && <span className="suna-model-tag-muted">已停用</span>}
                  </div>
                </div>
              </article>
            );
          })}
          <button type="button" className="suna-model-library-card suna-model-library-card-add" onClick={() => openCreate(defaultCreateType)}>
            <Plus size={22} />
            <span>添加{MODEL_TYPE_LABELS[defaultCreateType]}模型</span>
          </button>
          {filtered.length === 0 && (
            <div className="suna-model-library-empty">
              <p>{query.trim() ? "没有匹配的模型" : "尚未添加模型，点击「添加模型」开始配置"}</p>
              {query.trim() && (
                <Button variant="ghost" type="button" onClick={() => setQuery("")}>清空搜索</Button>
              )}
            </div>
          )}
        </div>
      )}

      {usageDetails && (
        <ModelInUseDialog details={usageDetails} onClose={() => setUsageDetails(null)} />
      )}
      <ModelDebugDrawer open={debugOpen} profiles={profiles} onClose={() => setDebugOpen(false)} />
      <ModelEditorDrawer
        open={drawerOpen}
        profile={editing}
        createType={editing ? inferProfileModelType(editing) : createType}
        onClose={() => {
          setDrawerOpen(false);
          setEditing(null);
        }}
        onSaved={() => void load()}
      />
    </div>
  );
}
