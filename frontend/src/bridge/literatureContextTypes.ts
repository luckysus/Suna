import type {LocalLiteratureEvidence} from './desktop';
export type LiteratureConditionKind='material_system'|'composition'|'heat_treatment'|'microstructure'|'test_method'|'test_conditions';
export interface CitedLiteratureCondition {kind:LiteratureConditionKind;text:string;quote:string;chunkId:string;chunkSha256:string}
export interface LiteratureContextSource {artifactId:string;evidenceSha256:string;field:string;value:number;unit:string;documentId:string;versionId:string;quote:string}
export interface ContextPage<T>{items:T[];offset:number;hasMore:boolean;readOnly:true}
export interface LiteratureContextSnapshot {schemaVersion:'literature-experimental-context.v1';sourceType:'literature';reviewed:true;evidenceArtifactId:string;evidenceSha256:string;documentId:string;versionId:string;documentSha256:string;conditions:(CitedLiteratureCondition&{location:Record<string,unknown>})[];scientificComparability:'not_assessed';rawSourceBytesVerified:false;unitsAndProcessOrderInferred:false}
export interface LiteratureContextRecord {artifactId:string;contentSha256:string;createdAt:string;snapshot:LiteratureContextSnapshot}
export interface LiteratureContextRequest {evidenceArtifactId:string;evidenceSha256:string;conditions:CitedLiteratureCondition[];confirmed:boolean}
export interface LiteratureContextReceipt {artifactId:string;contentSha256:string;sourceType:'literature';scientificComparability:'not_assessed'}
export interface LiteratureContextService {
 sources:(projectId:string,offset:number)=>Promise<ContextPage<LiteratureContextSource>>;
 list:(projectId:string,evidenceId:string,offset:number)=>Promise<ContextPage<LiteratureContextRecord>>;
 search:(query:string)=>Promise<LocalLiteratureEvidence[]>;
 save:(projectId:string,request:LiteratureContextRequest)=>Promise<LiteratureContextReceipt>;
 export:(fileName:string,bytes:number[])=>Promise<{path:string;bytes:number}|null>;
}
