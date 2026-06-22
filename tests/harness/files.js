const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

function projectFile(root, relativePath) {
  return path.join(root, relativePath);
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function readProjectJson(root, relativePath) {
  return readJson(projectFile(root, relativePath));
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function writeProjectJson(root, relativePath, value) {
  writeJson(projectFile(root, relativePath), value);
}

function tempProject(prefix) {
  const normalizedPrefix = prefix.endsWith("-") ? prefix : `${prefix}-`;
  return fs.mkdtempSync(path.join(os.tmpdir(), normalizedPrefix));
}

function hydrateRequest(root, request) {
  const hydrated = { ...request };
  for (const [key, value] of Object.entries(request)) {
    if (!key.endsWith("Ref") || typeof value !== "string" || key === "requestRef") continue;
    const targetKey = key.slice(0, -"Ref".length);
    if (targetKey in hydrated) continue;
    hydrated[targetKey] = readProjectJson(root, value);
  }
  return hydrated;
}

function requestFromCommand(data, root) {
  return data.request ?? hydrateRequest(root, readProjectJson(root, data.requestPath ?? data.requestRef));
}

module.exports = {
  hydrateRequest,
  projectFile,
  readJson,
  readProjectJson,
  requestFromCommand,
  tempProject,
  writeJson,
  writeProjectJson,
};
