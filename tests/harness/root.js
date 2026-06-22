const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "../..");
const cliPath = path.join(repoRoot, "dist", "cli.js");

function buildDist() {
  execFileSync("npm", ["run", "build"], { cwd: repoRoot, stdio: "inherit" });
}

function readRepoFile(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

module.exports = {
  buildDist,
  cliPath,
  readRepoFile,
  repoRoot,
};
