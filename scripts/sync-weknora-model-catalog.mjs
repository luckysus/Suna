#!/usr/bin/env node
/**
 * Sync WeKnora upstream model catalog → Suna desktop catalog.
 *
 * Sources (vendored under .weknora-upstream/):
 *   - catalog/models.generated.json  (model entries + match rules)
 *   - providers/*.go                 (vendor metadata: urls, types, extra fields)
 *   - assets/*.svg                   (vendor icons)
 *
 * Output:
 *   - src-tauri/resources/model_provider_catalog.json
 *   - src-tauri/resources/weknora_models.generated.json  (raw, for match resolve)
 *
 * Refresh from GitHub then regenerate:
 *   node scripts/fetch-weknora-catalog.mjs
 *
 * Or refresh .weknora-upstream/ manually, then:
 *   node scripts/sync-weknora-model-catalog.mjs
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const upstream = path.join(root, ".weknora-upstream");
const outCatalog = path.join(root, "src-tauri/resources/model_provider_catalog.json");
const outRaw = path.join(root, "src-tauri/resources/weknora_models.generated.json");

/** WeKnora provider id → Suna runtime ProviderKind */
const KIND_MAP = {
  aliyun: "qwen",
  deepseek: "deepseek",
  anthropic: "anthropic",
  siliconflow: "siliconflow",
  openai: "open_ai_compatible",
  openrouter: "open_ai_compatible",
  generic: "open_ai_compatible",
  azure_openai: "open_ai_compatible",
  gemini: "open_ai_compatible",
  moonshot: "open_ai_compatible",
  zhipu: "open_ai_compatible",
  jina: "open_ai_compatible",
  volcengine: "open_ai_compatible",
  hunyuan: "open_ai_compatible",
  lkeap: "open_ai_compatible",
  minimax: "open_ai_compatible",
  mimo: "open_ai_compatible",
  modelscope: "open_ai_compatible",
  qianfan: "open_ai_compatible",
  qiniu: "open_ai_compatible",
  longcat: "open_ai_compatible",
  gpustack: "open_ai_compatible",
  litellm: "open_ai_compatible",
  novita: "open_ai_compatible",
  nvidia: "open_ai_compatible",
  requesty: "open_ai_compatible",
  pinecone: "open_ai_compatible",
  huggingface_tei: "open_ai_compatible",
  cohere: "open_ai_compatible",
  weknoracloud: "open_ai_compatible",
};

const TYPE_MAP = {
  KnowledgeQA: "chat",
  Embedding: "embedding",
  Rerank: "rerank",
  VLLM: "vllm",
  ASR: "asr",
  chat: "chat",
  embedding: "embedding",
  rerank: "rerank",
  vllm: "vllm",
  asr: "asr",
};

function mapType(raw) {
  if (!raw) return "chat";
  return TYPE_MAP[raw] || String(raw).toLowerCase();
}

function read(file) {
  return fs.readFileSync(file, "utf8");
}

function extractStringConst(src, name) {
  const re = new RegExp(`const\\s+${name}\\s*=\\s*"([^"]+)"`);
  return src.match(re)?.[1] ?? null;
}

function extractMapBlock(src, field) {
  const start = src.indexOf(`${field}:`);
  if (start < 0) return {};
  const brace = src.indexOf("{", start);
  if (brace < 0) return {};
  let depth = 0;
  let end = brace;
  for (let i = brace; i < src.length; i++) {
    if (src[i] === "{") depth++;
    if (src[i] === "}") {
      depth--;
      if (depth === 0) {
        end = i;
        break;
      }
    }
  }
  const block = src.slice(brace + 1, end);
  const out = {};
  for (const m of block.matchAll(/types\.ModelType(\w+)\s*:\s*(\w+)/g)) {
    const type = mapType(m[1] === "KnowledgeQA" ? "KnowledgeQA" : m[1]);
    const constName = m[2];
    const value = extractStringConst(src, constName) || constName;
    // Prefer string literals assigned to named consts; fall back to inline
    out[type] = value.startsWith("http") || value.includes("{") ? value : extractStringConst(src, constName) || value;
  }
  // Also catch inline string URLs
  for (const m of block.matchAll(/types\.ModelType(\w+)\s*:\s*"([^"]+)"/g)) {
    out[mapType(m[1] === "KnowledgeQA" ? "KnowledgeQA" : m[1])] = m[2];
  }
  return out;
}

function extractModelTypes(src) {
  const start = src.indexOf("ModelTypes:");
  if (start < 0) return [];
  const slice = src.slice(start, start + 800);
  const types = [];
  for (const m of slice.matchAll(/types\.ModelType(\w+)/g)) {
    types.push(mapType(m[1] === "KnowledgeQA" ? "KnowledgeQA" : m[1]));
  }
  return [...new Set(types)];
}

function extractNames(src) {
  const m = src.match(/Names:\s*map\[string\]string\{([^}]*)\}/);
  if (!m) return {};
  const labels = {};
  for (const pair of m[1].matchAll(/"([^"]+)"\s*:\s*"([^"]+)"/g)) {
    labels[pair[1]] = pair[2];
  }
  return labels;
}

const EXTRA_KEY_ALIASES = {
  "models.ExtraAPIVersion": "api_version",
  "models.ExtraAPI": "api",
  "models.ExtraRemoteModelName": "remote_model_name",
  "models.ExtraThinkingControl": "thinking_control",
  "models.ExtraTruncatePromptTokens": "truncate_prompt_tokens",
  "models.ExtraScoreScale": "score_scale",
};

function extractExtraFields(src) {
  const fields = [];
  // ExtraFields: []ExtraField{ { Key: "api_version" | models.ExtraAPIVersion, ... }, ... }
  const start = src.indexOf("ExtraFields:");
  if (start < 0) return fields;
  const slice = src.slice(start, start + 6000);
  const chunks = slice.split(/\{\s*\n?\s*Key:/);
  for (let i = 1; i < chunks.length; i++) {
    const chunk = "Key:" + chunks[i].slice(0, 800);
    let key = chunk.match(/Key:\s*"([^"]+)"/)?.[1];
    if (!key) {
      const alias = chunk.match(/Key:\s*(models\.\w+)/)?.[1];
      key = EXTRA_KEY_ALIASES[alias] || null;
    }
    if (!key) continue;
    const label = chunk.match(/Label:\s*"([^"]+)"/)?.[1] || key;
    const required = /Required:\s*true/.test(chunk);
    const placeholder = chunk.match(/Placeholder:\s*"([^"]+)"/)?.[1] || "";
    const zhLabel = chunk.match(/Labels:\s*map\[string\]string\{[^}]*"zh-CN"\s*:\s*"([^"]+)"/)?.[1];
    const secret = /Secret:\s*true/.test(chunk) || /Type:\s*"password"/.test(chunk);
    fields.push({
      id: key,
      label: zhLabel || label,
      placeholder,
      required,
      secret: secret || undefined,
    });
  }
  return fields;
}

function extractCredentialLabel(src) {
  const start = src.indexOf("CredentialLabels:");
  if (start < 0) return null;
  const slice = src.slice(start, start + 1500);
  const label = slice.match(/Label:\s*"([^"]+)"/)?.[1];
  const zh = slice.match(/Labels:\s*map\[string\]string\{[^}]*"zh-CN"\s*:\s*"([^"]+)"/)?.[1];
  if (!label && !zh) return null;
  return { id: "api_key", label: zh || label, secret: true };
}

function parseProviderGo(filePath) {
  const src = read(filePath);
  const id =
    extractStringConst(src, src.match(/const\s+(\w+ID)\s*=/)?.[1] || "") ||
    path.basename(filePath, ".go").replace(/_/g, "");
  // Prefer XxxID const value
  const idConst = [...src.matchAll(/const\s+(\w+ID)\s*=\s*"([^"]+)"/g)].find((m) =>
    /ID$/.test(m[1]),
  );
  const providerId = idConst?.[2] || id;
  const name = src.match(/Name:\s*"([^"]+)"/)?.[1] || providerId;
  const description = src.match(/Description:\s*"([^"]+)"/)?.[1] || "";
  const website = src.match(/Website:\s*"([^"]+)"/)?.[1] || "";
  const order = Number(src.match(/Order:\s*(\d+)/)?.[1] || 100);
  const labels = extractNames(src);
  const defaultUrls = extractMapBlock(src, "DefaultBaseURLs");
  // Resolve named consts used as map values
  for (const [k, v] of Object.entries(defaultUrls)) {
    if (!v.startsWith("http") && !v.includes("{")) {
      const resolved = extractStringConst(src, v);
      if (resolved) defaultUrls[k] = resolved;
    }
  }
  const modelTypes = extractModelTypes(src);
  const extraFields = extractExtraFields(src);
  const credential = extractCredentialLabel(src);
  return {
    id: providerId,
    label: name,
    labels,
    description,
    website,
    order,
    default_urls: defaultUrls,
    model_types: modelTypes.length ? modelTypes : ["chat"],
    extra_fields: extraFields,
    credential_fields: credential
      ? [credential]
      : providerId === "generic"
        ? [{ id: "api_key", label: "API Key", secret: true }]
        : [{ id: "api_key", label: "API Key", secret: true }],
  };
}

function iconSvg(providerId) {
  const candidates = [
    path.join(upstream, "assets", `${providerId}.svg`),
    path.join(upstream, "assets", `${providerId.replace(/_/g, "")}.svg`),
  ];
  for (const file of candidates) {
    if (fs.existsSync(file)) {
      return fs.readFileSync(file, "utf8").replace(/\s+/g, " ").trim();
    }
  }
  return null;
}

function modelEntry(raw) {
  // WeKnora picker hides pattern-only rows (ID empty); keep those in the raw
  // file for resolve, not in the click-to-fill list.
  if (!raw.id || raw.deprecated) return null;
  const types = [];
  const mapped = mapType(raw.type);
  if (raw.type) types.push(mapped);
  else types.push("chat");
  // Vision chat models also usable as vllm in WeKnora pickers
  if ((raw.input || []).includes("image") && mapped === "chat") {
    types.push("vllm");
  }
  return {
    id: raw.id,
    name: raw.name || raw.id,
    types: [...new Set(types)],
    context_window: raw.context_window ?? undefined,
    max_output_tokens: raw.max_output_tokens ?? undefined,
    dimension: raw.dimension ?? undefined,
    input: raw.input || [],
    reasoning: raw.reasoning ?? undefined,
    source: raw.source || undefined,
  };
}

function sunaExtras() {
  return [
    {
      id: "ollama",
      label: "Ollama 本地",
      labels: { "zh-CN": "Ollama 本地" },
      description: "本机 Ollama（WeKnora source=local）",
      default_urls: {
        chat: "http://127.0.0.1:11434",
        embedding: "http://127.0.0.1:11434",
        vllm: "http://127.0.0.1:11434",
      },
      model_types: ["chat", "embedding", "vllm"],
      kind: "ollama",
      models: [],
      icon_svg: null,
      credential_fields: [],
      extra_fields: [],
      order: 5,
    },
    {
      id: "mineru",
      label: "MinerU",
      labels: { "zh-CN": "MinerU" },
      description: "文档解析（Suna 扩展，WeKnora 模型目录无此项）",
      default_urls: { document_parser: "https://mineru.net/api/v4" },
      model_types: ["document_parser"],
      kind: "mineru",
      models: [{ id: "mineru-v1", name: "MinerU Cloud", types: ["document_parser"] }],
      icon_svg: null,
      credential_fields: [{ id: "api_key", label: "API Key", secret: true }],
      extra_fields: [],
      order: 200,
    },
  ];
}

function main() {
  const generatedPath = path.join(upstream, "catalog/models.generated.json");
  if (!fs.existsSync(generatedPath)) {
    console.error("Missing", generatedPath);
    console.error("Download WeKnora catalog first (see script header).");
    process.exit(1);
  }
  const generated = JSON.parse(read(generatedPath));
  fs.writeFileSync(outRaw, JSON.stringify(generated, null, 2) + "\n");

  const providerFiles = fs
    .readdirSync(path.join(upstream, "providers"))
    .filter((f) => f.endsWith(".go") && f !== "builtin.go" && f !== "definition.go" && !f.includes("test") && f !== "compat.go");

  const providers = [];
  for (const file of providerFiles) {
    const meta = parseProviderGo(path.join(upstream, "providers", file));
    const entries = generated.providers[meta.id] || [];
    const models = entries.map(modelEntry).filter(Boolean);

    // Ensure model_types covers types present in models
    const fromModels = new Set(models.flatMap((m) => m.types));
    const modelTypes = [...new Set([...(meta.model_types || []), ...fromModels])];

    providers.push({
      id: meta.id,
      label: meta.label,
      labels: meta.labels,
      description: meta.description,
      website: meta.website || undefined,
      default_urls: meta.default_urls,
      model_types: modelTypes,
      kind: KIND_MAP[meta.id] || "open_ai_compatible",
      models,
      icon_svg: iconSvg(meta.id),
      credential_fields: meta.credential_fields,
      extra_fields: meta.extra_fields,
      order: meta.order,
      upstream_id: meta.id,
    });
  }

  for (const extra of sunaExtras()) {
    if (!providers.some((p) => p.id === extra.id)) providers.push(extra);
  }

  providers.sort((a, b) => (a.order ?? 100) - (b.order ?? 100) || a.id.localeCompare(b.id));

  const document = {
    version: 2,
    source: "Tencent/WeKnora internal/models/catalog/data/models.generated.json + providers/*.go",
    synced_at: new Date().toISOString(),
    providers: providers.map(({ order, upstream_id, website, ...rest }) => rest),
  };

  fs.writeFileSync(outCatalog, JSON.stringify(document, null, 2) + "\n");

  const modelCount = providers.reduce((n, p) => n + p.models.length, 0);
  console.log(
    `Synced ${providers.length} providers, ${modelCount} model entries → ${path.relative(root, outCatalog)}`,
  );
  console.log(`Raw WeKnora catalog → ${path.relative(root, outRaw)}`);
}

main();
