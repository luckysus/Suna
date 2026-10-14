export type Algorithm = "linear_regression" | "elasticnet" | "random_forest" | "hist_gradient_boosting" | "lightgbm" | "xgboost" | "svr" | "mlp" | "transformer";
export interface ModelNode {
  name: string; algorithm: Algorithm; targetColumn: number; featureColumns: number[];
  direction: "maximize" | "minimize"; unit: string | null;
  splitPolicy: {kind: "random" | "group" | "time"; validationFraction: number; seed?: number; groupColumn?: number; timeColumn?: number};
}
export interface Template {
  schemaVersion: "research-workflow-template.v1"; id: string; revision: number; name: string;
  models: ModelNode[]; variables: {name: string; unit?: string; fieldId?: string; min: number; max: number; fixedValue?: number}[];
  constraints: unknown[]; trials: number; seed: number; timeoutSeconds: number;
}
export interface Column {ordinal: number; originalName: string; canonicalField: string | null; inferredType: "number" | "text" | "date"; unit: string | null; min: number | null; max: number | null}
export interface Dataset {id: string; sourceName: string; sourceSha256: string; mappingState: "draft" | "ready"; columns: Column[]}
export interface SavedTemplate {template: Template; contentSha256: string; validationStatus: "saved_schema_only"; repositoryDryRunRequired: true}
export interface TemplateSummary {id: string; revision: number; name: string; contentSha256: string}
export interface FrozenInput {projectId: string; datasetId: string; datasetSha256: string}
export interface Preview {
  schemaVersion: "research-workflow-dry-run.v1"; projectId: string; datasetId: string; datasetSha256: string;
  templateId: string; templateRevision: number; dryRunSha256: string; networkRequired: boolean;
  validationStatus: "repository_validated_read_only";
  timeoutSeconds: number;
  validatedModels: {name: string; algorithm: string; featureNames: string[]; targetSchema: {name: string; unit?: string}; validRowCount: number; filteredRowCount: number; trainingPayloadSha256: string}[];
  materialRules: null | {ruleSetHash: string; ruleCount: number};
}
export interface TaskStatus {
  id: string; kind: string; state: "queued" | "running" | "waiting_external" | "paused" | "completed" | "failed" | "cancelled" | "interrupted";
  progress: number; attempt: number; error_code: string | null; cancel_requested: boolean; can_cancel: boolean; can_retry: boolean;
}
export interface WorkflowRun {
  projectId: string; task: TaskStatus; nodes: TaskStatus[]; templateId: string; templateRevision: number;
  definitionArtifactId: string; definitionContentSha256: string; templateContentSha256: string; approvedDeadline: string;
  result: null | {artifactId: string; contentSha256: string; optimizationTaskId: string; registeredModelIds: string[];
    trainingTaskIds: string[]; proposedPlanId: string; confirmationRequiredToAccept: true; measurementsCreated: false};
}
export interface Service {
  listDatasets(): Promise<Dataset[]>;
  listTemplates(): Promise<TemplateSummary[]>;
  getTemplate(id: string, revision: number): Promise<SavedTemplate>;
  saveTemplate(template: Template, expectedRevision: number): Promise<SavedTemplate>;
  dryRun(template: Template, input: FrozenInput): Promise<Preview>;
  start(request: FrozenInput & {templateId: string; templateRevision: number; dryRunSha256: string; nonce: string; confirmed: true}): Promise<WorkflowRun>;
  listRuns(projectId: string): Promise<WorkflowRun[]>;
  getRun(projectId: string, id: string): Promise<WorkflowRun>;
  cancelBackgroundTask(id: string): Promise<unknown>;
  retryBackgroundTask(id: string): Promise<unknown>;
  listenSchedulerProgress(fn: (event: {id: string}) => void): Promise<() => void>;
}
