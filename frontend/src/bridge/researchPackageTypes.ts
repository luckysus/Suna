export interface MissingDependency {
  resource_id: string; kind: string; status: "missing"; expected_sha256?: string; unit?: string;
}
export interface PackageSummary {
  schemaVersion: "research-package-manifest.v1"; scope: "metadata_only"; sourceProjectId: string;
  receiptSha256: string; metadataRecordCount: number; edgeCount: number; missingDependencies: MissingDependency[];
  assetsIncluded: false; provenanceMode: "references_only"; rightsStatus: "not_assessed";
  writesToLiveWorkspace: false; assetsRestored: false;
}
export interface PackagePreview extends PackageSummary {packageSha256: string; bytes: number}
export interface PackageSaved extends PackagePreview {writeVerified: true}
export interface PackageInspection extends PackagePreview {
  dryRun: {
    schemaVersion: "research-package-dry-run.v1"; receiptSha256: string; sourceProjectId: string;
    metadataRecordCount: number; edgeCount: number; missingDependencies: MissingDependency[];
    status: "metadata_only_missing_assets"; writesToLiveWorkspace: false; assetsRestored: false;
    idempotenceVerified: true; idMapping: Record<string, string>;
  };
}
export interface PackageService {
  preview(projectId: string): Promise<PackagePreview>;
  export(projectId: string, expectedSha256: string): Promise<PackageSaved | null>;
  inspectSelectedFile(): Promise<PackageInspection | null>;
}
