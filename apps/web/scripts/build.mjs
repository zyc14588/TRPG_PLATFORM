import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const output = path.join(root, "dist");
const config = JSON.parse(await readFile(path.join(root, "src/config.json"), "utf8"));

if (!Array.isArray(config.services) || config.services.length !== 5) {
  throw new Error("web configuration must define exactly five P01 services");
}
for (const service of config.services) {
  const url = new URL(service.url);
  if (!service.name || !["http:", "https:"].includes(url.protocol)) {
    throw new Error("web service configuration is invalid");
  }
}

await rm(output, { recursive: true, force: true });
await mkdir(path.join(output, "src"), { recursive: true });
await cp(path.join(root, "src/app.js"), path.join(output, "src/app.js"));
await cp(path.join(root, "src/health.js"), path.join(output, "src/health.js"));
await cp(path.join(root, "src/styles.css"), path.join(output, "src/styles.css"));
await cp(path.join(root, "src/config.json"), path.join(output, "config.json"));
await writeFile(
  path.join(output, "index.html"),
  await readFile(path.join(root, "index.html"), "utf8"),
);

console.log(`built @trpg/web ${config.version} with ${config.services.length} service checks`);
