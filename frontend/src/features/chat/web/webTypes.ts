import type { EvidenceItem, Message } from "../../../bridge/desktop";

export interface WebRecommendation {
  title: string;
  category?: string;
  summary?: string;
  details?: Record<string, unknown>;
  risks?: string[];
}

export interface WebPendingConfirmation {
  action_id: string;
  tool_name: string;
  title: string;
  permission: string;
  arguments: Record<string, unknown>;
  warning?: string;
}

export interface WebSource {
  index: number;
  title: string;
  url: string;
  site?: string;
  date?: string;
  snippet?: string;
}

export interface WebQueryRewrite {
  original_query: string;
  retrieval_query: string;
  changed?: boolean;
  alias_queries?: string[];
  multi_queries?: string[];
  hyde_used?: boolean;
  step_back_query?: string;
  decomposed_queries?: string[];
}

export interface WebResearchSnapshot {
  goal?: string;
  plan?: string;
  tasks?: string[];
  evidence?: string[];
  decisions?: string[];
  verification_issues?: string[];
  todo?: string[];
  status?: string;
  replan_count?: number;
}

export interface WebResponse {
  follow_up_questions: string[];
  suggested_questions: string[];
  recommendations: WebRecommendation[];
  pending_confirmations: WebPendingConfirmation[];
  evidence: EvidenceItem[];
  context_status: WebContextStatus;
  reasoning?: string;
  reasoning_ms?: number;
  web_sources: WebSource[];
  query_rewrite?: WebQueryRewrite | null;
  research?: WebResearchSnapshot | null;
  tool_calls?: unknown;
  run_id?: string;
  duration_ms?: number;
  started_at?: string;
  ended_at?: string;
}

export interface WebContextStatus {
  memory_count: number;
  skill_count: number;
  tool_count: number;
}

export interface WebMessage {
  role: "user" | "agent";
  content: string;
  response: WebResponse | null;
  streamEvidence: EvidenceItem[];
  reasoning?: string;
  reasoningMs?: number;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" ? value as Record<string, unknown> : null;
}

function asStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}

function asEvidence(value: unknown): EvidenceItem[] {
  return Array.isArray(value) ? value.filter((item): item is EvidenceItem => {
    const record = asRecord(item);
    return Boolean(record && typeof record.citation_number === "number" && record.chunk);
  }) : [];
}

function asRecommendations(value: unknown): WebRecommendation[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item) => {
    const record = asRecord(item);
    if (!record || typeof record.title !== "string") return [];
    return [{
      title: record.title,
      category: typeof record.category === "string" ? record.category : undefined,
      summary: typeof record.summary === "string" ? record.summary : undefined,
      details: asRecord(record.details) ?? undefined,
      risks: asStringArray(record.risks),
    }];
  });
}

function asConfirmations(value: unknown): WebPendingConfirmation[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item) => {
    const record = asRecord(item);
    if (!record || typeof record.action_id !== "string" || typeof record.title !== "string") return [];
    return [{
      action_id: record.action_id,
      tool_name: typeof record.tool_name === "string" ? record.tool_name : "",
      title: record.title,
      permission: typeof record.permission === "string" ? record.permission : "confirm",
      arguments: asRecord(record.arguments) ?? {},
      warning: typeof record.warning === "string" ? record.warning : undefined,
    }];
  });
}

function asWebSources(value: unknown): WebSource[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item, index) => {
    const record = asRecord(item);
    if (!record || typeof record.url !== "string" || typeof record.title !== "string") return [];
    return [{
      index: typeof record.index === "number" ? record.index : index + 1,
      title: record.title,
      url: record.url,
      site: typeof record.site === "string" ? record.site : undefined,
      date: typeof record.date === "string" ? record.date : undefined,
      snippet: typeof record.snippet === "string" ? record.snippet : undefined,
    }];
  });
}

function countArray(value: unknown): number {
  return Array.isArray(value) ? value.length : 0;
}

function responseContextStatus(record: Record<string, unknown>): WebContextStatus {
  const memory = asRecord(record.memory);
  const skills = asRecord(record.skills);
  return {
    memory_count: typeof memory?.selected_count === "number"
      ? memory.selected_count
      : countArray(memory?.selected),
    skill_count: countArray(skills?.loaded),
    tool_count: countArray(record.tool_calls),
  };
}

export function parseWebResponse(message: Message): WebResponse | null {
  if (!["agent", "assistant"].includes(message.role) || !message.response_json) return null;
  try {
    const record = asRecord(JSON.parse(message.response_json));
    if (!record) return null;
    const rewrite = asRecord(record.query_rewrite);
    const research = asRecord(record.research);
    return {
      follow_up_questions: asStringArray(record.follow_up_questions),
      suggested_questions: asStringArray(record.suggested_questions),
      recommendations: asRecommendations(record.recommendations),
      pending_confirmations: asConfirmations(record.pending_confirmations),
      evidence: asEvidence(record.evidence),
      context_status: responseContextStatus(record),
      reasoning: typeof record.reasoning === "string" ? record.reasoning : undefined,
      reasoning_ms: typeof record.reasoning_ms === "number" ? record.reasoning_ms : undefined,
      web_sources: asWebSources(record.web_sources),
      tool_calls: record.tool_calls,
      run_id: typeof record.run_id === "string" ? record.run_id : undefined,
      duration_ms: typeof record.duration_ms === "number" ? record.duration_ms : undefined,
      started_at: typeof asRecord(record.workflow)?.started_at === "string"
        ? String(asRecord(record.workflow)?.started_at)
        : typeof record.started_at === "string" ? record.started_at : undefined,
      ended_at: typeof asRecord(record.workflow)?.ended_at === "string"
        ? String(asRecord(record.workflow)?.ended_at)
        : typeof record.ended_at === "string" ? record.ended_at : undefined,
      query_rewrite: rewrite && typeof rewrite.original_query === "string" && typeof rewrite.retrieval_query === "string"
        ? {
          original_query: rewrite.original_query,
          retrieval_query: rewrite.retrieval_query,
          changed: rewrite.changed === true,
          alias_queries: asStringArray(rewrite.alias_queries),
          multi_queries: asStringArray(rewrite.multi_queries),
          hyde_used: rewrite.hyde_used === true,
          step_back_query: typeof rewrite.step_back_query === "string" ? rewrite.step_back_query : undefined,
          decomposed_queries: asStringArray(rewrite.decomposed_queries),
        }
        : null,
      research: research
        ? {
          goal: typeof research.goal === "string" ? research.goal : undefined,
          plan: typeof research.plan === "string" ? research.plan : undefined,
          tasks: asStringArray(research.tasks),
          evidence: asStringArray(research.evidence),
          decisions: asStringArray(research.decisions),
          verification_issues: asStringArray(research.verification_issues),
          todo: asStringArray(research.todo),
          status: typeof research.status === "string" ? research.status : undefined,
          replan_count: typeof research.replan_count === "number" ? research.replan_count : undefined,
        }
        : null,
    };
  } catch {
    return null;
  }
}

export function toWebMessage(message: Message): WebMessage {
  const response = parseWebResponse(message);
  return {
    role: message.role === "user" ? "user" : "agent",
    content: message.content,
    response,
    streamEvidence: response?.evidence ?? [],
    reasoning: response?.reasoning,
    reasoningMs: response?.reasoning_ms,
  };
}
