import { expect, it } from "vitest";
import { buildShapleyPlan, explainShapleyPlan } from "./sampledShapley";
it("assigns nonlinear interaction fairly and preserves efficiency against the chosen baseline", () => {
  const plan=buildShapleyPlan([2,3],[0,0]);
  const explanation=explainShapleyPlan(plan,plan.rows.map(([a,b])=>a*b),["C","temperature"],[2,3],"random_forest");
  expect(explanation.method).toBe("baseline_shapley_exact");
  expect(explanation.values.map((v)=>v.contribution)).toEqual([3,3]);
  expect(explanation.baseValue+explanation.values.reduce((sum,v)=>sum+v.contribution,0)).toBe(explanation.prediction);
});
it("seeded sampling is repeatable and recovers linear contributions with zero sampling variance", () => {
  const input=[1,2,3,4,5],baseline=[0,0,0,0,0];
  const plan=buildShapleyPlan(input,baseline);
  expect(plan).toEqual(buildShapleyPlan(input,baseline));
  const explanation=explainShapleyPlan(plan,plan.rows.map((r)=>r.reduce((sum,v,i)=>sum+(i+1)*v,0)),["a","b","c","d","e"],input,"svr");
  expect(explanation.method).toBe("baseline_shapley_sampled");
  expect(explanation.values.map((v)=>v.contribution)).toEqual([1,4,9,16,25]);
  expect(explanation.values.every((v)=>v.standardError===0)).toBe(true);
});
it("rejects invalid baselines and incomplete predictions", () => {
  expect(()=>buildShapleyPlan([1,2],[0])).toThrow();
  const plan=buildShapleyPlan([1],[0]);
  expect(()=>explainShapleyPlan(plan,[],["C"],[1],"svr")).toThrow();
});
