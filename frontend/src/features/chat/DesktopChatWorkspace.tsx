import { useEffect, useMemo, useRef, useState, type ChangeEvent, type FormEvent, type KeyboardEvent, type RefObject } from "react";
import type { Message } from "../../bridge/desktop";
import { useNativeFileDrop } from "../../hooks/useNativeFileDrop";
import {
  ArrowUp,
  Check,
  ChevronDown,
  Copy,
  CornerDownLeft,
  Download,
  FileJson,
  FileText,
  Globe,
  LoaderCircle,
  MessageSquarePlus,
  Pencil,
  Play,
  RotateCcw,
  ShieldAlert,
  SlidersHorizontal,
  Square,
  Wrench,
  X,
} from "lucide-react";
import { useLocale } from "../../i18n/locale";
import type { SectionId } from "../../app/navigation";
import type { PostgresKnowledgeSearchFilters } from "../../bridge/desktop";
import type { PermissionDecision } from "../../bridge/generated/protocol";
import LazyAIAnswerRenderer from "../../components/answer/LazyAIAnswerRenderer";
import ChatMessageList from "./ChatMessageList";
import { useAgentStore } from "../../stores/agentStore";
import { useChatStore } from "../../stores/chatStore";
import CitationPanel from "./CitationPanel";
import ChatModelParameters from "./ChatModelParameters";
import ModelSelector from "../settings/modelLibrary/ModelSelector";
import type { AgentPermissionView, AgentRunView } from "./agentEvents";
import type { ChatControllerProps } from "./chatController";
import AgentRunInspector from "./AgentRunInspector";
import { Input } from "../../components/ui/input";
import { Textarea } from "../../components/ui/textarea";
import { toWebMessage } from "./web/webTypes";
import WebTurnNavigator from "./web/WebTurnNavigator";
import { useAppearanceSettings } from "../../settings/appearance";
import { desktop } from "../../bridge/desktop";
import SunaLogo from "../../components/SunaLogo";
import { Button } from "../../components/ui/button";

function stateTone(state: string) {
  if (state === "completed") return "is-good";
  if (["failed", "cancelled", "interrupted"].includes(state)) return "is-danger";
  return "is-pending";
}

function stateLabel(
  state: AgentRunView["state"],
  t: (key: "contextPreparing" | "generating" | "runtimeReady" | "stopGenerating" | "chatError") => string,
) {
  if (state === "completed") return t("runtimeReady");
  if (["failed"].includes(state)) return t("chatError");
  if (["cancelled"].includes(state)) return t("stopGenerating");
  return ["created", "preparing", "awaiting_permission", "interrupted"].includes(state)
    ? t("contextPreparing")
    : t("generating");
}

function NativePermissionPanel({
  permissions,
  onResolve,
}: {
  permissions: AgentPermissionView[];
  onResolve: (permissionId: string, decision: PermissionDecision) => void;
}) {
  const { t } = useLocale();
  const pending = permissions.filter((permission) => permission.decision === null);
  if (pending.length === 0) return null;
  const actions: Array<{ decision: PermissionDecision; label: string }> = [
    { decision: "allow_once", label: t("allowOnce") },
    { decision: "allow_session", label: t("allowSession") },
    { decision: "allow_always", label: t("allowAlways") },
    { decision: "deny", label: t("deny") },
  ];

  return (
    <div className="suna-chat-permission-list" role="alert">
      {pending.map((permission) => (
        <div className="suna-chat-permission" key={permission.permissionId}>
          <div className="suna-chat-permission-heading">
            <ShieldAlert size={16} aria-hidden="true" />
            <div>
              <strong>{t("permissionRequired")}</strong>
              <span>{permission.summary}</span>
            </div>
          </div>
          <p>{permission.reason}</p>
          <div className="suna-chat-permission-actions">
            {actions.map(({ decision, label }) => (
              <Button
                type="button"
                variant={decision === "deny" ? "actionSecondary" : "action"}
                key={decision}
                onClick={() => onResolve(permission.permissionId, decision)}
              >
                {decision === "deny" ? <X size={14} aria-hidden="true" /> : <Check size={14} aria-hidden="true" />}
                {label}
              </Button>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

function NativeRunStatus({
  run,
  recovery,
  onResolvePermission,
  onRetry,
  onResume,
}: {
  run: AgentRunView | null;
  recovery: ChatControllerProps["recovery"];
  onResolvePermission: (permissionId: string, decision: PermissionDecision) => void;
  onRetry: () => void;
  onResume: () => void;
}) {
  const { t } = useLocale();
  if (!run) return null;
  const hasPendingPermission = run.permissions.some((permission) => permission.decision === null);
  const settled = ["completed", "failed", "cancelled", "interrupted"].includes(run.state);
  const canRetry = settled && run.state !== "completed";
  const canResume = recovery?.action.kind === "resume_from_checkpoint";
  if (settled && !hasPendingPermission && run.toolCalls.length === 0 && !canRetry && !canResume) return null;

  return (
    <div className="suna-chat-inline-status" aria-live="polite">
      <div className="suna-chat-inline-status-line">
        <span className={`suna-chat-inline-status-state ${stateTone(run.state)}`}>
          <Wrench size={13} aria-hidden="true" />
          {stateLabel(run.state, t)}
        </span>
        {run.toolCalls.length > 0 && <span>{t("agentToolCount", { count: run.toolCalls.length })}</span>}
        {run.taskProgress && <span>{run.taskProgress.kind} · {run.taskProgress.progress}%</span>}
        {canResume && (
          <Button variant="actionSecondary" type="button" onClick={onResume}>
            <Play size={13} aria-hidden="true" />{t("resumeAgentRun")}
          </Button>
        )}
        {canRetry && (
          <Button variant="actionSecondary" type="button" onClick={onRetry}>
            <RotateCcw size={13} aria-hidden="true" />{t("retryAgentRun")}
          </Button>
        )}
      </div>
      {run.toolCalls.length > 0 && (
        <div className="suna-chat-tool-trace" aria-label="Agent tools">
          {run.toolCalls.map((tool) => <span key={tool.toolCallId}>{tool.name} · {tool.progress}%</span>)}
        </div>
      )}
      <NativePermissionPanel permissions={run.permissions} onResolve={onResolvePermission} />
    </div>
  );
}

function ChatTurnNavigator({ scrollContainerRef }: { scrollContainerRef: RefObject<HTMLDivElement | null> }) {
  const messages = useChatStore((state) => state.messages);
  return <WebTurnNavigator messages={messages.map(toWebMessage)} scrollContainerRef={scrollContainerRef} />;
}

/** Provider 显示名（第 19 章：模型选择需展示 Provider）。 */
const PROVIDER_KIND_LABELS: Record<string, string> = {
  open_ai_compatible: "OpenAI 兼容",
  anthropic: "Anthropic",
  qwen: "Qwen",
  deepseek: "DeepSeek",
  ollama: "本地 Ollama",
  siliconflow: "SiliconFlow",
  mineru: "MinerU",
};

function ChatStreamingTurn({
  onResolvePermission,
  onRetry,
  onResume,
}: {
  onResolvePermission: (permissionId: string, decision: PermissionDecision) => void;
  onRetry: () => void;
  onResume: () => void;
}) {
  const { t } = useLocale();
  const pendingQuestion = useChatStore((state) => state.pendingQuestion);
  const streamingCitations = useChatStore((state) => state.streamingCitations);
  const agentRun = useAgentStore((state) => state.agentRun);
  const recovery = useAgentStore((state) => {
    const run = state.agentRun;
    if (!run) return null;
    return state.recoveredRuns.find((candidate) => candidate.run.id === run.runId) ?? null;
  });

  if (!pendingQuestion) return null;

  return (
    <>
      <div className="suna-chat-user-turn" data-agent-user-turn="pending">
        <div className="suna-chat-user-bubble">{pendingQuestion}</div>
      </div>
      <article className="suna-chat-assistant-turn is-streaming" aria-label="Suna">
        <div className="suna-chat-answer ai-markdown-body">
          <LazyAIAnswerRenderer answer={agentRun?.assistantText || t("contextPreparing")} literatureResults={[]} />
          {agentRun?.assistantText && <span className="ai-typing-cursor" aria-hidden="true" />}
        </div>
        {agentRun?.evidencePackId && streamingCitations.length > 0 && (
          <CitationPanel auditId={agentRun.evidencePackId} evidence={streamingCitations} />
        )}
      </article>
      <NativeRunStatus
        run={agentRun}
        recovery={recovery}
        onResolvePermission={onResolvePermission}
        onRetry={onRetry}
        onResume={onResume}
      />
    </>
  );
}

export default function DesktopChatWorkspace({
  onOpenSection: _onOpenSection,
  ...controller
}: ChatControllerProps & { onOpenSection?: (section: SectionId) => void }) {
  const { t } = useLocale();
  const { preferences } = useAppearanceSettings();
  const [modelMenuOpen, setModelMenuOpen] = useState(false);
  const [agentMenuOpen, setAgentMenuOpen] = useState(false);
  const [knowledgeMenuOpen, setKnowledgeMenuOpen] = useState(false);
  const [paramsOpen, setParamsOpen] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [sendShortcut, setSendShortcut] = useState("Ctrl+Enter");
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const imageInputRef = useRef<HTMLInputElement>(null);
  const messagesRef = useRef<HTMLDivElement>(null);
  const composerRef = useRef<HTMLFormElement>(null);
  const messageCount = useChatStore((state) => state.messages.length);
  const messageActions = useMemo(
    () => ({
      onEditMessage: (message: Message) => {
        controller.onDraftChange(message.content);
        inputRef.current?.focus();
      },
      onFollowUp: (question: string) => {
        controller.onDraftChange(question);
        inputRef.current?.focus();
      },
      onResolvePermission: controller.onResolvePermission,
    }),
    [controller.onDraftChange, controller.onResolvePermission],
  );
  const activeProfile = controller.chatProfiles.find((profile) => profile.id === controller.activeChatProfileId);
  const selectedModel = activeProfile?.model_id || activeProfile?.display_name || "本地模型";
  const providerLabel = activeProfile ? PROVIDER_KIND_LABELS[activeProfile.kind] ?? activeProfile.kind : "未配置";

  useEffect(() => {
    let active = true;
    if (typeof desktop.getSetting !== "function") return () => { active = false; };
    void desktop.getSetting("ui.shortcuts").then((raw) => {
      if (!active || !raw) return;
      try {
        const value = JSON.parse(raw) as Record<string, unknown>;
        if (typeof value.send === "string" && value.send) setSendShortcut(value.send);
      } catch {
        // Keep the safe default when settings are unavailable or malformed.
      }
    }).catch(() => undefined);
    return () => { active = false; };
  }, []);

  const onComposerKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    const parts = sendShortcut.toLowerCase().split("+");
    const key = parts[parts.length - 1] ?? "enter";
    const matches = event.key.toLowerCase() === key
      && event.ctrlKey === parts.includes("ctrl")
      && event.shiftKey === parts.includes("shift")
      && event.altKey === parts.includes("alt")
      && event.metaKey === parts.includes("meta");
    if (sendShortcut !== "未设置" && matches) {
      event.preventDefault();
      event.currentTarget.form?.requestSubmit();
    }
  };

  const encodeBytes = (bytes: Uint8Array) => {
    let binary = "";
    const chunkSize = 0x8000;
    for (let offset = 0; offset < bytes.length; offset += chunkSize) {
      binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
    }
    return btoa(binary);
  };

  const onAttachmentFiles = (files: FileList | File[]) => {
    const accepted = Array.from(files).filter((file) => file.type.startsWith("image/") || /\.(pdf|docx?|xlsx?|csv|txt|md|markdown|json|html?)$/i.test(file.name));
    if (accepted.length === 0) return;
    void Promise.all(accepted.map((file) => new Promise<{ name: string; mime: string; data: string } | null>((resolve) => {
      const reader = new FileReader();
      reader.onload = () => {
        const value = reader.result;
        const data = typeof value === "string"
          ? (file.type.startsWith("text/") || /\.(csv|txt|md|markdown|json|html?)$/i.test(file.name)
            ? encodeBytes(new TextEncoder().encode(value))
            : (value.includes(",") ? value.slice(value.indexOf(",") + 1) : value))
          : encodeBytes(new Uint8Array(value as ArrayBuffer));
        resolve(data ? { name: file.name || "attachment", mime: file.type || "application/octet-stream", data } : null);
      };
      reader.onerror = () => resolve(null);
      if (file.type.startsWith("text/") || /\.(csv|txt|md|markdown|json|html?)$/i.test(file.name)) reader.readAsText(file);
      else reader.readAsDataURL(file);
    }))).then((items) => {
      const next = items.filter((item): item is { name: string; mime: string; data: string } => item !== null);
      if (next.length > 0) controller.onAttachmentsChange([...controller.attachments, ...next]);
    });
  };

  const onFileInputChange = (event: ChangeEvent<HTMLInputElement>) => {
    if (event.target.files) onAttachmentFiles(event.target.files);
    event.target.value = "";
  };

  const nativeMime = (name: string) => {
    const extension = name.toLowerCase().split(".").pop() || "";
    const known: Record<string, string> = {
      pdf: "application/pdf", doc: "application/msword", docx: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
      xls: "application/vnd.ms-excel", xlsx: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
      csv: "text/csv", txt: "text/plain", md: "text/markdown", markdown: "text/markdown", json: "application/json", html: "text/html", htm: "text/html",
      png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", webp: "image/webp", gif: "image/gif",
    };
    return known[extension] || "application/octet-stream";
  };

  const addNativeAttachments = (paths: string[]) => {
    const next = paths.filter((path) => /\.(pdf|docx?|xlsx?|csv|txt|md|markdown|json|html?|png|jpe?g|webp|gif)$/i.test(path)).map((path) => ({
      name: path.split(/[\\/]/).pop() || "attachment", mime: nativeMime(path), data: "", path,
    }));
    if (next.length > 0) controller.onAttachmentsChange([...controller.attachments, ...next]);
  };
  useNativeFileDrop(composerRef, { onFiles: addNativeAttachments, onHover: setDragging });

  const pickNativeAttachments = async () => {
    try {
      const selected = await desktop.openFileDialog({
        multiple: true,
        directory: false,
        title: "添加 Agent 附件",
        filters: [{ name: "支持的文档附件", extensions: ["pdf", "doc", "docx", "xls", "xlsx", "csv", "txt", "md", "markdown", "json", "html", "htm"] }],
      });
      const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
      addNativeAttachments(paths);
    } catch {
      // Keep the composer usable when the native picker is unavailable.
    }
  };

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (controller.pendingQuestion !== null) {
      void controller.onSteer(controller.draft);
      return;
    }
    void controller.onSubmit(event);
  };

  return (
    <section className={`suna-chat ${preferences.showAgentPanel ? "" : "has-agent-panel-hidden"}`} aria-label={t("chatAriaLabel")}>
      <main className="suna-chat-main">
        {controller.selectedConversation && (
          <header className="suna-chat-header" style={{ justifyContent: "space-between" }}>
            <div>
              <p className="suna-eyebrow">{t("steelRuntime")}</p>
              <h2>{controller.selectedConversation.title}</h2>
            </div>
            <div className="suna-chat-header-actions">
              <Button size="icon" type="button" aria-label={t("chatExportMarkdown")} title={t("chatExportMarkdown")} onClick={() => controller.onExportConversation("markdown")}><Download size={16} /></Button>
              <Button size="icon" type="button" aria-label={t("chatExportJson")} title={t("chatExportJson")} onClick={() => controller.onExportConversation("json")}><FileJson size={16} /></Button>
            </div>
          </header>
        )}

        {(controller.error || controller.notice) && (
          <div className="suna-chat-main-alerts">
            {controller.error && <div className="suna-knowledge-alert" role="alert">{controller.error}</div>}
            {controller.notice && <div className="suna-knowledge-notice" role="status">{controller.notice}</div>}
          </div>
        )}

        <div ref={messagesRef} className="suna-chat-messages" aria-live="polite">
          {controller.loadingMessages ? (
            <div className="suna-chat-empty"><LoaderCircle size={20} className="suna-spin" /><span>{t("loading")}</span></div>
          ) : messageCount === 0 && controller.pendingQuestion === null ? (
            <div className="suna-chat-empty suna-chat-empty-large suna-new-welcome">
              <div className="suna-welcome-brand">
                <span className="suna-welcome-mark"><SunaLogo size={52} title="Suna" /></span>
                <div><strong>Suna</strong><small>{t("welcomeSubtitle")}</small></div>
              </div>
              <h1>{t("welcomeHeading")} <em>Suna</em></h1>
              <p>{t("welcomeDesc")}</p>
              <div
                className="suna-welcome-suggestion-box"
                role="button"
                tabIndex={0}
                onClick={() => controller.onDraftChange(t("welcomeExampleQuestion"))}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    controller.onDraftChange(t("welcomeExampleQuestion"));
                  }
                }}
              >
                <span className="suna-welcome-compat-copy">{t("welcomeStartQuestion")}</span>
                <span className="suna-welcome-compat-copy">{t("welcomeExampleQuestion")}</span>
                <b>→</b>
              </div>
              <div className="suna-welcome-cards">
                {[
                  { icon: "◈", title: t("welcomeCardMaterial"), text: t("welcomeCardMaterialDesc") },
                  { icon: "▤", title: t("welcomeCardLiterature"), text: t("welcomeCardLiteratureDesc") },
                  { icon: "▥", title: t("welcomeCardData"), text: t("welcomeCardDataDesc") },
                  { icon: "△", title: t("welcomeCardPrediction"), text: t("welcomeCardPredictionDesc") },
                  { icon: "✥", title: t("welcomeCardOptimization"), text: t("welcomeCardOptimizationDesc") },
                  { icon: "♜", title: t("welcomeCardExperiment"), text: t("welcomeCardExperimentDesc") },
                ].map((item) => (
                  <button
                    type="button"
                    className="suna-welcome-card"
                    key={item.title}
                    onClick={() => controller.onDraftChange(`${item.title}: `)}
                  >
                    <span>{item.icon}</span>
                    <strong>{item.title}</strong>
                    <small>{item.text}</small>
                    <b>→</b>
                  </button>
                ))}
              </div>
            </div>
          ) : (
            <>
              <ChatMessageList
                loading={controller.pendingQuestion !== null}
                messageActions={messageActions}
                scrollContainerRef={messagesRef}
                onLoadOlderMessages={controller.onLoadOlderMessages}
                isOlderMessagesPaginationActive={controller.isOlderMessagesPaginationActive}
              />
              <ChatStreamingTurn
                onResolvePermission={controller.onResolvePermission}
                onRetry={controller.onRetry}
                onResume={controller.onResume}
              />
              {!controller.pendingQuestion && (
                <NativeRunStatus
                  run={controller.agentRun}
                  recovery={controller.recovery}
                  onResolvePermission={controller.onResolvePermission}
                  onRetry={controller.onRetry}
                  onResume={controller.onResume}
                />
              )}
            </>
          )}
          <ChatTurnNavigator scrollContainerRef={messagesRef} />
        </div>

        <form
          ref={composerRef}
          className={`suna-chat-composer ${dragging ? "is-dragging" : ""}`}
          data-testid="desktop-agent-composer"
          onSubmit={submit}
          onDragOver={(event) => { event.preventDefault(); if (!dragging) setDragging(true); }}
          onDragLeave={(event) => {
            const next = event.relatedTarget as Node | null;
            if (next && event.currentTarget.contains(next)) return;
            setDragging(false);
          }}
          onDrop={(event) => {
            event.preventDefault();
            setDragging(false);
            if (event.dataTransfer?.files?.length) onAttachmentFiles(event.dataTransfer.files);
          }}
        >
          {dragging && <div className="suna-chat-dropzone" role="region" aria-label="拖拽文件到输入框"><FileText size={18} /><span>松开即可添加 PDF、Excel、CSV、Word 或图片</span></div>}
          {controller.attachments.length > 0 && (
            <div className="mb-2 flex flex-wrap gap-2" aria-label="已添加附件">
              {controller.attachments.map((attachment, index) => (
                <div className="group/attachment relative h-16 w-16 overflow-hidden rounded-lg border border-[var(--suna-line)] bg-[var(--suna-bg-soft)]" key={`${attachment.name}-${index}`}>
                  {attachment.mime.startsWith("image/") ? <img src={`data:${attachment.mime};base64,${attachment.data}`} alt={attachment.name} className="h-full w-full object-cover" /> : <div className="flex h-full w-full flex-col items-center justify-center gap-1 p-1 text-[var(--suna-text-muted)]"><FileText size={22} /><span className="max-w-full truncate text-[10px]">{attachment.name}</span></div>}
                  <button type="button" className="absolute right-0.5 top-0.5 flex h-5 w-5 items-center justify-center rounded-full bg-black/55 text-white" aria-label={`移除图片 ${attachment.name}`} title={`移除图片 ${attachment.name}`} onClick={() => controller.onAttachmentsChange(controller.attachments.filter((_, itemIndex) => itemIndex !== index))}><X size={12} /></button>
                </div>
              ))}
            </div>
          )}
          <Textarea
            ref={inputRef}
            value={controller.draft}
            onChange={(event) => controller.onDraftChange(event.target.value)}
            onKeyDown={onComposerKeyDown}
            onPaste={(event) => {
              const files = event.clipboardData?.files;
              if (files && files.length > 0) {
                event.preventDefault();
                onAttachmentFiles(files);
              }
            }}
            aria-label={t("inputMessage")}
            placeholder={t("askPlaceholder")}
            rows={3}
            disabled={false}
          />
          <div className="suna-chat-composer-footer">
            <div className="suna-chat-composer-tools">
              <button type="button" className={`suna-chat-composer-tool ${controller.smartSearchEnabled || controller.autoKnowledgeSearchEnabled ? "is-active" : ""}`} aria-label={t("smartSearch")} aria-pressed={controller.smartSearchEnabled || controller.autoKnowledgeSearchEnabled} title={t("smartSearchTitle")} onClick={controller.onToggleSmartSearch} disabled={controller.pendingQuestion !== null}><Globe size={15} /><span>{t("smartSearch")}</span></button>
              <button type="button" className="suna-chat-composer-tool" aria-label={t("imageButton")} title={t("imageButton")} onClick={() => imageInputRef.current?.click()} disabled={controller.pendingQuestion !== null}><MessageSquarePlus size={15} /><span>{t("imageButton")}</span></button>
              <button type="button" className="suna-chat-composer-tool" aria-label={t("attachments")} title={t("attachments")} onClick={() => void pickNativeAttachments()} disabled={controller.pendingQuestion !== null}><FileText size={15} /><span>{t("attachments")}</span></button>
              <input ref={imageInputRef} type="file" accept="image/*" multiple hidden onChange={onFileInputChange} />
              <input ref={fileInputRef} type="file" accept="image/*,.pdf,.doc,.docx,.xls,.xlsx,.csv,.txt,.md,.markdown,.json,.html,.htm" multiple hidden onChange={onFileInputChange} />
            </div>
            <div className="suna-chat-composer-right">
              {controller.pendingQuestion !== null && (
                <>
                  <button
                    type="button"
                    className="suna-chat-composer-tool"
                    aria-label={t("followUp")}
                    title={t("chatFollowUpAria")}
                    disabled={!controller.draft.trim()}
                    onClick={() => void controller.onFollowUp(controller.draft)}
                  >
                    <CornerDownLeft size={15} aria-hidden="true" /><span>{t("followUp")}</span>
                  </button>
                  <button
                    type="button"
                    className="suna-chat-composer-tool"
                    aria-label={t("steer")}
                    title={t("chatSteerAria")}
                    disabled={!controller.draft.trim()}
                    onClick={() => void controller.onSteer(controller.draft)}
                  >
                    <ArrowUp size={15} aria-hidden="true" /><span>{t("steer")}</span>
                  </button>
                </>
              )}
              <div className="suna-chat-model-picker">
                {modelMenuOpen && (
                  <ModelSelector
                    profiles={controller.chatProfiles}
                    activeId={controller.activeChatProfileId}
                    filterType="chat"
                    onSelect={(profileId) => {
                      setModelMenuOpen(false);
                      controller.onSelectChatProfile(profileId);
                    }}
                  />
                )}
                <button type="button" className="suna-chat-model-button" aria-label={t("chatSwitchModel")} title={t("chatSwitchModel")} aria-expanded={modelMenuOpen} onClick={() => { setModelMenuOpen((open) => !open); setParamsOpen(false); }} disabled={controller.pendingQuestion !== null}>
                  <span>{selectedModel}</span><ChevronDown size={14} className={modelMenuOpen ? "is-open" : undefined} />
                </button>
              </div>
              <div className="suna-chat-model-picker">
                {paramsOpen && <ChatModelParameters providerLabel={providerLabel} modelLabel={selectedModel} />}
                <button type="button" className="suna-chat-model-button" aria-label={t("chatModelParams")} title={t("chatModelParamsDesc")} aria-expanded={paramsOpen} onClick={() => { setParamsOpen((open) => !open); setModelMenuOpen(false); setAgentMenuOpen(false); setKnowledgeMenuOpen(false); }} disabled={controller.pendingQuestion !== null}>
                  <SlidersHorizontal size={14} /><span>{t("parameters")}</span>
                </button>
              </div>
              <div className="suna-chat-model-picker">
                {agentMenuOpen && (
                  <div className="suna-chat-model-menu" role="menu">
                    <button type="button" role="menuitem" className={controller.activeAgentId === null ? "is-active" : ""} onClick={() => { setAgentMenuOpen(false); controller.onSelectAgent(null); }}>
                      <span>{t("autoAgentSelect")}</span>{controller.activeAgentId === null && <Check size={14} />}
                    </button>
                    {controller.agentProfiles.map((profile) => (
                      <button type="button" role="menuitem" className={profile.id === controller.activeAgentId ? "is-active" : ""} key={profile.id} onClick={() => { setAgentMenuOpen(false); controller.onSelectAgent(profile.id); }}>
                        <span>{profile.name}</span>{profile.id === controller.activeAgentId && <Check size={14} />}
                      </button>
                    ))}
                  </div>
                )}
                <button type="button" className="suna-chat-model-button" aria-label={t("chatSelectAgent")} title={t("chatSelectAgent")} aria-expanded={agentMenuOpen} onClick={() => { setAgentMenuOpen((open) => !open); setModelMenuOpen(false); setParamsOpen(false); }} disabled={controller.pendingQuestion !== null}>
                  <span>{controller.activeAgentId ? controller.agentProfiles.find((profile) => profile.id === controller.activeAgentId)?.name ?? controller.activeAgentId : t("autoAgent")}</span><ChevronDown size={14} className={agentMenuOpen ? "is-open" : undefined} />
                </button>
              </div>
              <div className="suna-chat-model-picker">
                {knowledgeMenuOpen && (
                  <div className="suna-chat-model-menu suna-chat-knowledge-menu" role="menu" aria-label={t("chatScopeFilters")}>
                    {controller.knowledgeBases.length === 0 ? <span className="suna-chat-model-empty">{t("noKnowledgeBases")}</span> : controller.knowledgeBases.map((base) => {
                      const checked = controller.selectedKnowledgeBaseIds.includes(base.id);
                      return <button type="button" role="menuitemcheckbox" aria-checked={checked} className={checked ? "is-active" : ""} key={base.id} onClick={() => controller.onSelectKnowledgeBases(checked ? controller.selectedKnowledgeBaseIds.filter((id) => id !== base.id) : [...controller.selectedKnowledgeBaseIds, base.id])}><span>{base.name}</span>{checked && <Check size={14} />}</button>;
                    })}
                    <div className="suna-chat-knowledge-filters" role="group" aria-label={t("chatScopeFilters")}>
                      <span className="suna-chat-knowledge-filters-title">{t("searchScope")}</span>
                      {([
                        ["document_type", t("filterDocType")],
                        ["material", t("filterMaterial")],
                        ["process", t("filterProcess")],
                        ["property", t("filterProperty")],
                        ["year", t("filterYear")],
                        ["tag", t("filterTag")],
                      ] as const).map(([key, label]) => (
                        <Input
                          key={key}
                          aria-label={label}
                          placeholder={label}
                          inputMode={key === "year" ? "numeric" : undefined}
                          value={controller.knowledgeFilters[key] ?? ""}
                          onChange={(event) => {
                            const raw = event.target.value;
                            const digits = raw.replace(/[^0-9]/g, "");
                            const value = key === "year" ? (digits ? Number(digits) : undefined) : raw;
                            controller.onSelectKnowledgeFilters({ ...controller.knowledgeFilters, [key]: value } as PostgresKnowledgeSearchFilters);
                          }}
                        />
                      ))}
                      <button type="button" className="suna-chat-knowledge-clear" onClick={() => controller.onSelectKnowledgeFilters({})}>{t("chatClearFilters")}</button>
                    </div>
                  </div>
                )}
                <button type="button" className="suna-chat-model-button" aria-label={t("chatSelectKnowledge")} title={t("chatSelectKnowledge")} aria-expanded={knowledgeMenuOpen} onClick={() => { setKnowledgeMenuOpen((open) => !open); setModelMenuOpen(false); setAgentMenuOpen(false); setParamsOpen(false); }} disabled={controller.pendingQuestion !== null}>
                  <span>{controller.selectedKnowledgeBaseIds.length === 0 ? t("chatNoKnowledge") : controller.selectedKnowledgeBaseIds.length === controller.knowledgeBases.length ? t("chatAllKnowledge") : t("chatKnowledgeCount", { count: controller.selectedKnowledgeBaseIds.length })}</span><ChevronDown size={14} className={knowledgeMenuOpen ? "is-open" : undefined} />
                </button>
              </div>
              <button type={controller.pendingQuestion ? "button" : "submit"} className={`suna-chat-send-button ${controller.pendingQuestion ? "is-stop" : ""}`} aria-label={controller.pendingQuestion ? t("stopGenerating") : t("send")} title={controller.pendingQuestion ? t("stopGenerating") : t("send")} disabled={!controller.pendingQuestion && !controller.draft.trim() && controller.attachments.length === 0} onClick={controller.pendingQuestion ? controller.onCancel : undefined}>
                {controller.pendingQuestion ? <Square size={15} fill="currentColor" /> : <ArrowUp size={20} strokeWidth={2.6} />}
              </button>
            </div>
          </div>
        </form>
      </main>

      {preferences.showAgentPanel && <AgentRunInspector
          run={controller.agentRun}
          recovery={controller.recovery}
          showToolDetails={preferences.showToolDetails}
          onResolvePermission={controller.onResolvePermission}
          onRetry={controller.onRetry}
          onResume={controller.onResume}
        />}
    </section>
  );
}
