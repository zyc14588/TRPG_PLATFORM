import { cp, mkdir, readFile, rm, stat } from "node:fs/promises";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const output = resolve(root, "dist");

if (relative(root, output).startsWith(`..${sep}`) || output === root) {
  throw new Error("Refusing to clean an unsafe build path.");
}

const packageJson = JSON.parse(await readFile(join(root, "package.json"), "utf8"));
const config = JSON.parse(await readFile(join(root, "app.config.json"), "utf8"));

if (config.version !== packageJson.version) {
  throw new Error("app.config.json version must match the web package version.");
}
if (!Array.isArray(config.services) || config.services.length !== 5) {
  throw new Error("app.config.json must define exactly five services.");
}

const expectedServices = [
  ["api", "API", "api-server", 8080],
  ["realtime", "Realtime", "realtime-server", 8081],
  ["agent-worker", "Agent Worker", "agent-worker", 8082],
  ["admin", "Admin", "admin-server", 8083],
  ["migration", "Migration", "migration-runner", 8084],
];

for (const [index, [id, name, processName, port]] of expectedServices.entries()) {
  const service = config.services[index];
  if (service?.id !== id || service?.name !== name || service?.processName !== processName) {
    throw new Error(`Service ${index + 1} must be ${name}.`);
  }
  const url = new URL(service.healthBaseUrl);
  if (url.protocol !== "http:" || url.hostname !== "127.0.0.1" || Number(url.port) !== port || url.pathname !== "/") {
    throw new Error(`${name} must use loopback port ${port}.`);
  }
}

await rm(output, { recursive: true, force: true });
await mkdir(output, { recursive: true });
await cp(join(root, "index.html"), join(output, "index.html"));
await cp(join(root, "app.config.json"), join(output, "app.config.json"));
await cp(join(root, "src"), join(output, "src"), { recursive: true });

const expectedFiles = ["index.html", "app.config.json", "src/app.js", "src/health.js", "src/styles.css"];
for (const file of expectedFiles) {
  const source = join(root, file);
  const built = join(output, file);
  if (!(await stat(built)).isFile()) throw new Error(`Missing build artifact: ${file}`);
  if (!Buffer.from(await readFile(source)).equals(Buffer.from(await readFile(built)))) {
    throw new Error(`Build artifact differs from source: ${file}`);
  }
}

console.log(`Built @coc-ai-trpg/web v${packageJson.version} (${expectedFiles.length} verified files).`);
