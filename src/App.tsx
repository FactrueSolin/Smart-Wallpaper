import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type AppStatus = {
  name: string;
  version: string;
  backend: {
    integrated: boolean;
    message: string;
  };
};

export default function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    invoke<AppStatus>("app_status")
      .then((result) => {
        if (!cancelled) {
          setStatus(result);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(String(err));
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <main className="app-shell">
      <section className="hero">
        <p className="eyebrow">Tauri + React</p>
        <h1>智能壁纸</h1>
        <p className="lead">
          当前项目已经切换为 Tauri 桌面应用结构，现有 Rust 壁纸模块作为后端集成在
          <code>src-tauri</code> 中。
        </p>
      </section>

      <section className="status-grid">
        <article className="card">
          <span className="card-label">应用状态</span>
          <strong>{status?.name ?? "正在连接后端"}</strong>
          <p>{status ? `版本 ${status.version}` : "等待 Tauri 命令返回"}</p>
        </article>

        <article className="card">
          <span className="card-label">后端模块</span>
          <strong>{status?.backend.integrated ? "已接入" : "未就绪"}</strong>
          <p>{status?.backend.message ?? error ?? "正在检查 wallpaper manager"}</p>
        </article>
      </section>

      <section className="next-steps">
        <h2>下一步</h2>
        <p>现在可以继续把屏幕列表、壁纸设置、目录扫描等命令逐步暴露到前端。</p>
      </section>
    </main>
  );
}
