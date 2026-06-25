const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "../../../..");
const referenceRoot = path.join(repoRoot, "src", "ts", "reference");
const cliPath = path.join(referenceRoot, "dist", "cli.js");

function buildDist() {
  execFileSync("npm", ["run", "build"], { cwd: referenceRoot, stdio: "inherit" });
}

function readRepoFile(relativePath) {
  const referencePath = path.join(referenceRoot, relativePath);
  if (fs.existsSync(referencePath)) {
    return fs.readFileSync(referencePath, "utf8");
  }
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

module.exports = {
  buildDist,
  cliPath,
  readRepoFile,
  referenceRoot,
  repoRoot,
};
