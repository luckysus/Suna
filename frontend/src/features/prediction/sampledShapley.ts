import type { ShapExplanation } from "../../bridge/desktop";

export interface ShapleyPlan { rows: number[][]; paths: Array<{ order: number[]; indices: number[] }>; exact: boolean }

/** Shapley attribution for the game f(masked input, fixed training-mean baseline).
 * This is model agnostic baseline attribution, not conditional/causal SHAP.
 * Enumerate all permutations up to four features; otherwise sample seeded orders.
 */
export function buildShapleyPlan(input: number[], baseline: number[], seed = 42): ShapleyPlan {
  if (!input.length || input.length > 64 || baseline.length !== input.length || [...input, ...baseline].some((n) => !Number.isFinite(n))) throw new Error("Shapley 解释需要 1–64 个有限输入与对应的训练均值基线");
  const indices = input.map((_, i) => i);
  let state = seed >>> 0;
  const random = () => { state = (Math.imul(state, 1664525) + 1013904223) >>> 0; return state / 4294967296; };
  const permutations = (remaining: number[]): number[][] => remaining.length ? remaining.flatMap((n) => permutations(remaining.filter((i) => i !== n)).map((rest) => [n, ...rest])) : [[]];
  const exact = input.length <= 4;
  const orders = exact ? permutations(indices) : Array.from({ length: 32 }, () => {
    const order = [...indices];
    for (let i = order.length-1; i > 0; i -= 1) { const j = Math.floor(random()*(i+1)); [order[i], order[j]] = [order[j], order[i]]; }
    return order;
  });
  const rows: number[][] = [];
  const known = new Map<string, number>();
  const rowIndex = (row: number[]) => {
    const key = JSON.stringify(row);
    const existing = known.get(key);
    if (existing !== undefined) return existing;
    const index = rows.length; rows.push([...row]); known.set(key, index); return index;
  };
  const paths = orders.map((order) => {
    const row = [...baseline];
    const positions = [rowIndex(row)];
    for (const i of order) { row[i] = input[i]; positions.push(rowIndex(row)); }
    return { order, indices: positions };
  });
  return { rows, paths, exact };
}

export function explainShapleyPlan(plan: ShapleyPlan, predictions: number[], names: string[], input: number[], modelType: string): ShapExplanation {
  if (predictions.length !== plan.rows.length || predictions.some((p) => !Number.isFinite(p)) || names.length !== input.length) throw new Error("Shapley 预测结果与请求维度不一致");
  const samples = names.map(() => [] as number[]);
  for (const path of plan.paths) path.order.forEach((feature, i) => samples[feature].push(predictions[path.indices[i+1]] - predictions[path.indices[i]]));
  const contributions = samples.map((values) => values.reduce((a,b) => a+b,0)/values.length);
  const total = contributions.reduce((a,b) => a+Math.abs(b),0);
  const first = plan.paths[0];
  return {
    modelType, method: plan.exact ? "baseline_shapley_exact" : "baseline_shapley_sampled", baseline: "training_mean", permutationCount: plan.paths.length,
    baseValue: predictions[first.indices[0]], prediction: predictions[first.indices[first.indices.length-1]],
    values: names.map((feature,i) => {
      const mean = contributions[i], values = samples[i];
      const standardError = plan.exact || values.length < 2 ? 0 : Math.sqrt(values.reduce((sum,v) => sum+(v-mean)**2,0)/(values.length-1)/values.length);
      return { feature, value: input[i], contribution: mean, share: total ? Math.abs(mean)/total : 0, standardError };
    }),
  };
}
