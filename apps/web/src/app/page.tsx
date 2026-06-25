import { backendPlaceholder } from "../lib/backend";

export default function HomePage() {
  const backend = backendPlaceholder();

  return (
    <main style={{ margin: "0 auto", maxWidth: 880, padding: "48px 24px" }}>
      <h1>TRPG Platform</h1>
      <p>Phase 0 bootstrap shell. 正式 UI、地图和 Agent 卡片将在后续阶段实现。</p>
      <section aria-labelledby="backend-heading">
        <h2 id="backend-heading">Backend connection placeholder</h2>
        <p>API base: {backend.baseUrl}</p>
        <ul>
          {backend.endpoints.map((endpoint) => (
            <li key={endpoint}>{endpoint}</li>
          ))}
        </ul>
      </section>
    </main>
  );
}
