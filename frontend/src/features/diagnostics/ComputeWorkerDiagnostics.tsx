import { useState } from "react";
import { desktop } from "../../bridge/desktop";
import { Button } from "../../components/ui/button";
import "./diagnostics.css";

export default function ComputeWorkerDiagnostics() {
  const [result, setResult] = useState<Awaited<ReturnType<typeof desktop.diagnoseComputeWorker>> | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const probe = async () => {
    setBusy(true);
    setError("");
    try {
      setResult(await desktop.diagnoseComputeWorker());
    } catch {
      setError("计算 Worker 诊断不可用，请检查安装并重试");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="suna-diagnostics-worker-card" aria-label="离线计算依赖诊断">
      <h2>离线计算 Worker</h2>
      <p>检查本机 Worker、协议和实际依赖加载，不发送数据。预测、优化和图像统计需要安装包附带 Worker；AI 对话、远端向量服务和公开文献检索分别需要网络。</p>
      <Button disabled={busy} onClick={() => void probe()}>
        {busy ? "正在检查本机计算依赖…" : "检查计算 Worker"}
      </Button>
      {error && <p role="alert">{error}</p>}
      {result && (
        <>
          <p role="status">
            {result.state === "ready" ? "计算依赖可用" : result.state === "degraded" ? "部分计算依赖不可用" : "计算 Worker 不可用"} {result.errorCode ?? ""} · Worker {result.workerVersion ?? "未读取"} · 协议 {result.protocolVersion ?? "未读取"}
          </p>
          {result.dependencies && (
            <div style={{ overflowX: "auto" }}>
              <table>
                <thead>
                  <tr>
                    <th>依赖</th>
                    <th>实际加载</th>
                    <th>版本</th>
                  </tr>
                </thead>
                <tbody>
                  {result.dependencies.map((d) => (
                    <tr key={d.name}>
                      <td>{d.name}</td>
                      <td>{d.status === "ready" ? "可用" : d.errorCode}</td>
                      <td>{d.version ?? "未提供分发元数据"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          <p>检查通过表示依赖可加载，具体模型与安装包仍需真实任务验证。缺依赖时按安装说明修复，不会自动修改 Python 环境。</p>
        </>
      )}
    </section>
  );
}
