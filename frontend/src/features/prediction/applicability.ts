export type ApplicabilityState = "within_domain" | "warning" | "outside_domain" | "unknown";
export interface PredictionInputFeature { name: string; unit?: string | null }
export interface ApplicabilityRow {
  row_index: number;
  state: ApplicabilityState;
  reasons: Array<{ state: ApplicabilityState; code: string; feature?: string }>;
}
export interface ApplicabilityResult { rows: ApplicabilityRow[]; support_sha256: string | null }

export function applicabilityRow(result: ApplicabilityResult | null | undefined, index: number): ApplicabilityRow {
  return result?.rows.find(row => row.row_index === index) ?? {
    row_index: index, state: "unknown", reasons: [{ state: "unknown", code: "legacy_model_without_support" }],
  };
}
export function applicabilityLabel(state: ApplicabilityState): string {
  return { within_domain: "通过训练范围筛查", warning: "需要复核", outside_domain: "超出训练范围", unknown: "适用域未知" }[state];
}
export function applicabilityReason(code: string): string {
  const labels: Record<string, string> = {
    outside_training_range: "超过训练集数值范围",
    missing_unit: "训练或输入单位未记录",
    unit_conversion_required: "需先转换为模型训练单位",
    missing_dataset_sha256: "训练数据哈希未记录",
    legacy_model_without_support: "旧模型未保存支持域信息",
    imputed_with_training_mean: "已用训练均值填补缺失项",
    unverified_training_scope: "无法确认统计量来自训练集",
    missing_training_limits: "该字段缺少训练支持统计",
    missing_input_feature: "输入缺少该字段",
    missing_input_value: "输入缺少该字段数值",
    unsupported_artifact_contract: "模型支持域版本尚不兼容",
    missing_model_identity: "模型版本信息未记录",
    missing_training_support: "模型缺少训练支持域数据",
  };
  return labels[code] ?? "缺少可核验的支持域证据";
}

export function parseBatchInput(text: string, modelNames: string[]): { rows: number[][]; inputSchema?: PredictionInputFeature[] } {
  const input: unknown = JSON.parse(text);
  let records: unknown = input;
  let names = modelNames;
  let inputSchema: PredictionInputFeature[] | undefined;
  if (input && typeof input === "object" && !Array.isArray(input)) {
    const wrapper = input as Record<string, unknown>;
    if (!Array.isArray(wrapper.schema) || wrapper.schema.length !== modelNames.length) throw new Error("输入 schema 必须包含全部模型特征");
    inputSchema = wrapper.schema.map((field: unknown) => {
      if (!field || typeof field !== "object" || Array.isArray(field)) throw new Error("特征 schema 格式错误");
      const value = field as Record<string, unknown>;
      if (typeof value.name !== "string" || !modelNames.includes(value.name)) throw new Error("输入包含未声明的模型特征");
      if (value.unit != null && (typeof value.unit !== "string" || !value.unit.trim() || value.unit.length > 64)) throw new Error("输入单位格式错误");
      return { name: value.name, unit: value.unit as string | null | undefined };
    });
    names = inputSchema.map(field => field.name);
    if (new Set(names).size !== names.length) throw new Error("输入 schema 特征重复");
    records = wrapper.rows;
  }
  if (!Array.isArray(records) || !records.length || records.length > 10000) throw new Error("请输入 1–10000 行 JSON 数组");
  const rows = records.map((row: unknown, i: number) => {
    const values = Array.isArray(row) ? row : row && typeof row === "object" ? names.map(name => (row as Record<string, unknown>)[name]) : [];
    if (values.length !== names.length || values.some(value => typeof value !== "number" || !Number.isFinite(value))) throw new Error(`第 ${i + 1} 行必须包含全部模型特征的有限数值`);
    return values as number[];
  });
  return { rows, ...(inputSchema ? { inputSchema } : {}) };
}
