#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const http = require("node:http");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "../..");

execFileSync("npm", ["run", "build"], { cwd: repoRoot, stdio: "inherit" });

async function main() {
  const { checkDeploymentPreview } = await import(path.join(repoRoot, "dist", "core", "deployment", "runtime.js"));

  const badServer = await startServer({
    "/": [
      200,
      "text/html",
      '<!doctype html><html><body><div id="root"></div><script type="module" src="/src/App.tsx"></script></body></html>',
    ],
    "/src/App.tsx": [200, "text/javascript", "var _s = $RefreshSig$(); export default function App() {}"],
  });
  try {
    const health = await checkDeploymentPreview(specFor(`http://127.0.0.1:${badServer.port}`));
    assert.equal(health.status, "unhealthy");
    assert.match(health.error ?? "", /React Fast Refresh preamble/);
  } finally {
    await badServer.close();
  }

  const goodServer = await startServer({
    "/": [
      200,
      "text/html",
      '<!doctype html><html><body><div id="root"></div><script type="module" src="/assets/index.js"></script></body></html>',
    ],
    "/assets/index.js": [200, "text/javascript", "document.getElementById('root').textContent = 'ready';"],
  });
  try {
    const health = await checkDeploymentPreview(specFor(`http://127.0.0.1:${goodServer.port}`));
    assert.equal(health.status, "healthy");
    assert.equal(health.error, null);
  } finally {
    await goodServer.close();
  }

  console.log("deploy preview validation checks passed");
}

function specFor(url) {
  return {
    runtimeContract: {
      previewPath: "/",
    },
    runtime: {
      url,
      healthcheck: {
        attempts: 1,
        intervalMs: 0,
        timeoutMs: 2_000,
      },
    },
  };
}

function startServer(routes) {
  const server = http.createServer((request, response) => {
    const route = routes[request.url] ?? [404, "text/plain", "not found"];
    response.writeHead(route[0], { "content-type": route[1] });
    response.end(route[2]);
  });
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      resolve({
        port: address.port,
        close: () => new Promise((closeResolve) => server.close(closeResolve)),
      });
    });
  });
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
