export interface EvidenceQualityCheck {
  evidenceSha256:string;artifactId:string;documentId:string;versionId:string;chunkId:string;
  field:string;value:number;unit:string;
  indexedSourceState:'matched'|'missing'|'changed';
  observedIndexSha256:string|null;
  doiState:'linked'|'not_recorded'|'link_missing';locationRecorded:boolean;
  unitState:'normalized'|'unknown_field'|'incompatible_or_overflow';
  canonicalField:string|null;canonicalUnit:string|null;normalizedValue:number|null;
  rawSourceBytesVerified:false;comparability:'conditions_not_recorded';
}
export interface EvidenceQualityReport {
  schemaVersion:'literature-evidence-quality.v1';algorithmVersion:'index-traceability.v1';
  projectId:string;scope:'reviewed_numeric_evidence';sourceType:'literature';registrySha256:string;
  reviewedEvidenceCount:number;
  counts:{indexedReferenceMatched:number;missingIndexedSource:number;changedIndexedSource:number;
    doiNotRecorded:number;doiLinkMissing:number;locationNotRecorded:number;unknownField:number;incompatibleUnit:number};
  checks:EvidenceQualityCheck[];limitations:string[];
}
export interface EvidenceQualityPreview {readOnly:true;reportSha256:string;report:EvidenceQualityReport}
export interface EvidenceQualityArchive {artifactId:string;contentSha256:string;reportSha256:string}
export interface EvidenceQualityService {
  preview(projectId:string):Promise<EvidenceQualityPreview>;
  archive(projectId:string,expectedSha256:string):Promise<EvidenceQualityArchive>;
}
