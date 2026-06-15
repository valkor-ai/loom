#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { ensureLoomUserInstall } = require("./lib/loom-user-install");

const repoRoot = path.resolve(__dirname, "..");
const opencodePluginRoot = path.join(repoRoot, "plugins", "opencode");
const opencodeSourceRoot = path.join(opencodePluginRoot, ".opencode");
const commandSourceRoot = path.join(opencodeSourceRoot, "commands");
const pluginSourceRoot = path.join(opencodeSourceRoot, "plugins");
const referenceSourceRoot = path.join(opencodeSourceRoot, "references");
const deployReferenceSourceRoot = path.join(repoRoot, "plugins", "shared", "loom-deploy", "references");
const opencodeConfigRoot = process.env.OPENCODE_CONFIG_HOME || path.join(process.env.HOME || "", ".config", "opencode");
const commandInstallRoot = path.join(opencodeConfigRoot, "commands");
const pluginInstallRoot = path.join(opencodeConfigRoot, "plugins");
const referenceInstallRoot = path.join(opencodeConfigRoot, "references");
const deployReferenceInstallRoot = path.join(opencodeConfigRoot, "loom-deploy", "references");
const legacyCommandInstallRoot = path.join(opencodeConfigRoot, "command");
const legacyDeployReferenceRoot = path.join(opencodeConfigRoot, "loomline-deploy");
const cliTarget = path.join(repoRoot, "dist", "cli.js");

if (!fs.existsSync(cliTarget)) {
  throw new Error("dist/cli.js does not exist. Run npm run build first.");
}
if (!fs.existsSync(commandSourceRoot)) {
  throw new Error("plugins/opencode/.opencode/commands does not exist.");
}
if (!fs.existsSync(pluginSourceRoot)) {
  throw new Error("plugins/opencode/.opencode/plugins does not exist.");
}
if (!fs.existsSync(deployReferenceSourceRoot)) {
  throw new Error("plugins/shared/loom-deploy/references does not exist.");
}

const removedLegacyArtifacts = removeLegacyOpencodeArtifacts();
fs.mkdirSync(commandInstallRoot, { recursive: true });
fs.mkdirSync(pluginInstallRoot, { recursive: true });
fs.mkdirSync(referenceInstallRoot, { recursive: true });
copyCommand("loom.md");
copyCommand("loom-deploy.md");
copyPlugin("loom.js");
const installedReferences = copyReferences();
fs.rmSync(deployReferenceInstallRoot, { recursive: true, force: true });
copyDirectory(deployReferenceSourceRoot, deployReferenceInstallRoot);
assertExists(path.join(deployReferenceInstallRoot, "node.md"));
const removedLegacyCommands = [
  removeLegacyCommand("loom.md"),
  removeLegacyCommand("loom-deploy.md"),
  removeLegacyCommand("loomline.md"),
  removeLegacyCommand("loomline-deploy.md"),
].filter(Boolean);
const userInstall = ensureLoomUserInstall({
  adapter: "opencode",
  repoRoot,
  pluginInstallRoot: opencodeConfigRoot,
});

const stamp = {
  pluginName: "loom",
  source: opencodePluginRoot,
  installRoot: opencodeConfigRoot,
  installedCommands: [
    path.join(commandInstallRoot, "loom.md"),
    path.join(commandInstallRoot, "loom-deploy.md"),
  ],
  installedPlugins: [
    path.join(pluginInstallRoot, "loom.js"),
  ],
  installedReferences,
  installedDeployReferencesRoot: deployReferenceInstallRoot,
  removedLegacyArtifactCount: removedLegacyArtifacts.length + removedLegacyCommands.length,
  cli: {
    cliTarget,
    launcherPath: userInstall.launcherPath,
    launcherRef: userInstall.launcherRef,
    userInstall,
  },
  refreshedAt: new Date().toISOString(),
};
fs.writeFileSync(path.join(opencodeConfigRoot, ".loom-opencode-refresh.json"), `${JSON.stringify(stamp, null, 2)}\n`);
console.log(JSON.stringify(stamp, null, 2));

function copyCommand(name) {
  const source = path.join(commandSourceRoot, name);
  const target = path.join(commandInstallRoot, name);
  if (!fs.existsSync(source)) {
    throw new Error(`Missing opencode command source: ${source}`);
  }
  fs.copyFileSync(source, target);
}

function copyPlugin(name) {
  const source = path.join(pluginSourceRoot, name);
  const target = path.join(pluginInstallRoot, name);
  if (!fs.existsSync(source)) {
    throw new Error(`Missing opencode plugin source: ${source}`);
  }
  fs.copyFileSync(source, target);
}

function copyReferences() {
  if (!fs.existsSync(referenceSourceRoot)) {
    return [];
  }
  const installed = [];
  for (const entry of fs.readdirSync(referenceSourceRoot, { withFileTypes: true })) {
    const source = path.join(referenceSourceRoot, entry.name);
    const target = path.join(referenceInstallRoot, entry.name);
    fs.rmSync(target, { recursive: true, force: true });
    if (entry.isDirectory()) {
      fs.mkdirSync(target, { recursive: true });
      copyDirectory(source, target);
      installed.push(target);
    } else if (entry.isFile()) {
      fs.copyFileSync(source, target);
      installed.push(target);
    }
  }
  return installed;
}

function copyDirectory(source, target) {
  for (const entry of fs.readdirSync(source, { withFileTypes: true })) {
    const sourcePath = path.join(source, entry.name);
    const targetPath = path.join(target, entry.name);
    if (entry.isDirectory()) {
      fs.mkdirSync(targetPath, { recursive: true });
      copyDirectory(sourcePath, targetPath);
    } else if (entry.isFile()) {
      fs.mkdirSync(path.dirname(targetPath), { recursive: true });
      fs.copyFileSync(sourcePath, targetPath);
    }
  }
}

function assertExists(filePath) {
  if (!fs.existsSync(filePath)) {
    throw new Error(`Expected opencode adapter file was not copied: ${filePath}`);
  }
}

function removeLegacyCommand(name) {
  const target = path.join(legacyCommandInstallRoot, name);
  if (!fs.existsSync(target)) {
    return null;
  }
  fs.rmSync(target);
  return target;
}

function removeLegacyOpencodeArtifacts() {
  const removed = [];
  for (const target of [
    path.join(commandInstallRoot, "loomline.md"),
    path.join(commandInstallRoot, "loomline-deploy.md"),
    path.join(pluginInstallRoot, "loomline.js"),
    path.join(referenceInstallRoot, "loomline"),
    legacyDeployReferenceRoot,
    path.join(opencodeConfigRoot, ".loomline-opencode-refresh.json"),
  ]) {
    if (fs.existsSync(target)) {
      fs.rmSync(target, { recursive: true, force: true });
      removed.push(target);
    }
  }
  return removed;
}
