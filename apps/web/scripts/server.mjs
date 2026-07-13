import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
import { extname, isAbsolute, relative, resolve, sep } from "node:path";
import { createServer } from "node:http";

const pairs = [];
for (let index = 2; index < process.argv.length; index += 1) {
  const value = process.argv[index];
  const next = process.argv[index + 1];
  if (value.startsWith("--") && next && !next.startsWith("--")) pairs.push([value.slice(2), next]);
}
const args = Object.fromEntries(pairs);

const root = resolve(process.cwd(), args.root ?? "dist");
const host = process.env.WEB_HOST ?? "127.0.0.1";
const port = Number(process.env.WEB_PORT ?? args.port ?? 4173);
const maxRequests = Number(process.env.WEB_MAX_REQUESTS ?? 0);

if (!Number.isInteger(port) || port < 1 || port > 65_535) throw new Error("WEB_PORT must be a valid port.");
if (!Number.isInteger(maxRequests) || maxRequests < 0) throw new Error("WEB_MAX_REQUESTS must be a non-negative integer.");

const mimeTypes = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".png": "image/png",
};

let handledRequests = 0;
const server = createServer(async (request, response) => {
  try {
    const pathname = decodeURIComponent(new URL(request.url ?? "/", "http://localhost").pathname);
    const requested = pathname === "/" ? "index.html" : pathname.replace(/^\/+/, "");
    const file = resolve(root, requested);
    const relativePath = relative(root, file);
    if (isAbsolute(relativePath) || relativePath === ".." || relativePath.startsWith(`..${sep}`)) {
      response.writeHead(403).end("Forbidden");
      return;
    }

    const metadata = await stat(file);
    if (!metadata.isFile()) throw new Error("not_a_file");
    response.writeHead(200, {
      "Content-Type": mimeTypes[extname(file)] ?? "application/octet-stream",
      "Content-Length": metadata.size,
      "Cache-Control": "no-store",
      "X-Content-Type-Options": "nosniff",
    });
    if (request.method === "HEAD") response.end();
    else createReadStream(file).pipe(response);
  } catch {
    response.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" }).end("Not found");
  } finally {
    response.once("finish", () => {
      handledRequests += 1;
      if (maxRequests > 0 && handledRequests >= maxRequests) server.close();
    });
  }
});

server.listen(port, host, () => {
  console.log(`COC AI TRPG web available at http://${host}:${port} from ${root}`);
});

for (const signal of ["SIGINT", "SIGTERM"]) process.on(signal, () => server.close());
