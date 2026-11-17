import type { EvidenceItem } from "../../bridge/desktop";
import type { AnswerReferenceResult } from "../../components/answer/AnswerRenderer";

/** Shared evidence → answer citation mapping for stream + history. */
export function evidenceToLiteratureResults(
  items: EvidenceItem[],
): AnswerReferenceResult[] {
  return items.map((item) => ({
    paper_name: item.chunk.source_name || item.chunk.document_id || `证据 ${item.citation_number}`,
    header_path: item.chunk.source_location.kind === "heading"
      ? item.chunk.source_location.path.join(" / ")
      : "",
    content: item.chunk.text || "",
    similarity_score: item.chunk.rerank_score ?? item.chunk.rrf_score ?? 0,
  }));
}
