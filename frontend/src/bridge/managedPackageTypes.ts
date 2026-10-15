import type {MissingDependency,PackageSummary} from './researchPackageTypes';
export interface ManagedAsset {
  resourceId:string;role:'dataset_source'|'image_original'|'model_artifact';extension:string;sha256:string;bytes:number;
}
export interface ManagedInventory {
  sourceProjectId:string;scope:'managed_assets_inventory';rawContentIncluded:false;rightsIndependentlyVerified:false;
  assets:{resourceId:string;role:ManagedAsset['role'];name:string;sha256:string;bytes:number|null;availability:'eligible_unverified'|'unavailable_or_external'|'unsupported_or_over_budget'}[];
}
export interface ManagedSummary {
  schemaVersion:'research-managed-package.v1';scope:'managed_assets_isolated';packageSha256:string;sourceProjectId:string;
  metadata:PackageSummary;assets:ManagedAsset[];assetCount:number;assetBytes:number;missingDependencies:MissingDependency[];
  rightsStatus:'user_declared_authorized';rightsIndependentlyVerified:false;writesToLiveWorkspace:false;modelsActivated:false;measurementsCreated:false;
}
export interface ManagedPreview extends ManagedSummary {bytes:number}
export interface ManagedSaved extends ManagedPreview {writeVerified:true}
export interface ManagedIsolation extends ManagedSummary {
  isolationId:string;idMapping:Record<string,string>;status:'isolated_assets_stored';assetsRestoredToIsolation:true;
  assetsRestoredToLiveWorkspace:false;readyForComputation:false;reopenVerified:true;
}
export interface ManagedSelection {projectId:string;assetIds:string[];confirmedAssetRights:boolean}
export interface ManagedService {
  inventory(projectId:string):Promise<ManagedInventory>;
  preview(request:ManagedSelection):Promise<ManagedPreview>;
  export(request:ManagedSelection,expectedSha256:string):Promise<ManagedSaved|null>;
  inspect():Promise<ManagedSummary|null>;
  importIsolated(expectedSha256:string,confirmed:true):Promise<ManagedIsolation|null>;
}
