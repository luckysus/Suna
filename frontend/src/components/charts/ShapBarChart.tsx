import type { ShapExplanation } from "../../bridge/desktop";

/**
 * SHAP 贡献条形图（第 37 章）。
 *
 * 展示线性精确 SHAP 或明确标记的基线置换 Shapley。
 * 两者均满足效率性，不把特征重要性当作 Shapley 贡献。
 */
export default function ShapBarChart({ explanation }: { explanation: ShapExplanation }) {
  const maxAbs = explanation.values.reduce((max, item) => Math.max(max, Math.abs(item.contribution)), 0);

  return (
    <div className="suna-shap">
      <div className="suna-shap-summary">
        <span>基准值 <strong>{explanation.baseValue.toFixed(3)}</strong></span>
        <span>预测值 <strong>{explanation.prediction.toFixed(3)}</strong></span>
        <span>模型 <strong>{explanation.modelType}</strong></span>
      </div>
      <ul className="suna-shap-list">
        {explanation.values.map((item) => {
          const ratio = maxAbs > 0 ? Math.abs(item.contribution) / maxAbs : 0;
          const positive = item.contribution >= 0;
          return (
            <li key={item.feature}>
              <span className="suna-shap-feature">{item.feature}</span>
              <span className="suna-shap-track">
                <i
                  className={positive ? "is-positive" : "is-negative"}
                  style={{ width: `${Math.max(1, ratio * 100)}%` }}
                />
              </span>
              <span className={`suna-shap-value ${positive ? "is-positive" : "is-negative"}`}>
                {positive ? "+" : ""}{item.contribution.toFixed(3)}
              </span>
              <span className="suna-shap-share">{(item.share * 100).toFixed(1)}%</span>
            </li>
          );
        })}
      </ul>
      <p className="suna-correlation-legend">
        {explanation.method ? "模型无关基线 Shapley：各贡献之和加上基线预测等于当前预测。结果依赖基线，不能视为因果效应；采样标准误不含训练数据或模型不确定性。" : "精确 SHAP：各特征贡献之和加上基准值等于模型预测值。"}
      </p>
    </div>
  );
}
