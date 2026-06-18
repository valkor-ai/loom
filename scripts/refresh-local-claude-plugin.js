#!/usr/bin/env node
"use strict";

const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const { ensureLoomUserInstall } = require("./lib/loom-user-install");

const repoRoot = path.resolve(__dirname, "..");
const claudePluginRoot = path.join(repoRoot, "plugins", "claude-code");
const sharedDeployReferenceSourceRoot = path.join(repoRoot, "plugins", "shared", "loom-deploy", "references");
const sharedDeliveryReferenceSourceRoot = path.join(repoRoot, "plugins", "shared", "loom", "references", "delivery");
const manifestPath = path.join(claudePluginRoot, ".claude-plugin", "plugin.json");
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const pluginName = manifest.name;
const legacyPluginName = "loomline";
const claudeHome = process.env.CLAUDE_HOME || path.join(process.env.HOME || "", ".claude");
const installRoot = path.join(claudeHome, "skills", pluginName);
const legacyInstallRoot = path.join(claudeHome, "skills", legacyPluginName);
const legacyPluginDataPath = path.join(claudeHome, "plugins", "data", `${legacyPluginName}-skills-dir`);
const commandsRoot = path.join(claudeHome, "commands");
const cliTarget = path.join(repoRoot, "dist", "cli.js");

if (!pluginName) {
  throw new Error("plugins/claude-code/.claude-plugin/plugin.json must contain name.");
}
if (!fs.existsSync(cliTarget)) {
  throw new Error("dist/cli.js does not exist. Run npm run build first.");
}

execFileSync("claude", ["plugin", "validate", "--strict", claudePluginRoot], {
  cwd: repoRoot,
  stdio: "inherit",
});

const removedLegacyArtifacts = removeLegacyClaudeArtifacts();
fs.rmSync(installRoot, { recursive: true, force: true });
fs.mkdirSync(installRoot, { recursive: true });
copyDirectory(claudePluginRoot, installRoot, claudePluginRoot);
installSharedDeployReferences(path.join(installRoot, "skills", "loom-deploy", "references"));
installSharedDeliveryReferences(path.join(installRoot, "skills", "loom", "references", "delivery"));
installGlobalCommands();

assertExists(path.join(installRoot, ".claude-plugin", "plugin.json"));
assertExists(path.join(installRoot, "commands", "loom.md"));
assertExists(path.join(installRoot, "commands", "loom-deploy.md"));
assertExists(path.join(installRoot, "hooks", "hooks.json"));
assertExists(path.join(installRoot, "hooks", "loom-workflow-guard.js"));
assertExists(path.join(installRoot, "skills", "loom", "SKILL.md"));
assertExists(path.join(installRoot, "skills", "loom", "references", "delivery", "repair.md"));
assertExists(path.join(installRoot, "skills", "loom-deploy", "SKILL.md"));
assertExists(path.join(installRoot, "skills", "loom-deploy", "references", "node.md"));
assertExists(path.join(commandsRoot, "loom.md"));
assertExists(path.join(commandsRoot, "loom-deploy.md"));
const userInstall = ensureLoomUserInstall({
  adapter: "claude",
  repoRoot,
  pluginInstallRoot: installRoot,
});

const stamp = {
  pluginName,
  source: claudePluginRoot,
  installRoot,
  installedCommands: [
    path.join(commandsRoot, "loom.md"),
    path.join(commandsRoot, "loom-deploy.md"),
  ],
  removedLegacyArtifactCount: removedLegacyArtifacts.length,
  cli: {
    cliTarget,
    launcherPath: userInstall.launcherPath,
    launcherRef: userInstall.launcherRef,
    userInstall,
  },
  refreshedAt: new Date().toISOString(),
};
fs.writeFileSync(path.join(installRoot, ".loom-claude-refresh.json"), `${JSON.stringify(stamp, null, 2)}\n`);
console.log(JSON.stringify(stamp, null, 2));

function installGlobalCommands() {
  const sourceCommandsRoot = path.join(claudePluginRoot, "commands");
  fs.mkdirSync(commandsRoot, { recursive: true });
  for (const name of ["loom.md", "loom-deploy.md"]) {
    fs.copyFileSync(path.join(sourceCommandsRoot, name), path.join(commandsRoot, name));
  }
}

function installSharedDeployReferences(targetRoot) {
  if (!fs.existsSync(sharedDeployReferenceSourceRoot)) {
    throw new Error(`Shared deploy references source does not exist: ${sharedDeployReferenceSourceRoot}`);
  }
  fs.rmSync(targetRoot, { recursive: true, force: true });
  copyDirectory(sharedDeployReferenceSourceRoot, targetRoot, sharedDeployReferenceSourceRoot);
}

function installSharedDeliveryReferences(targetRoot) {
  if (!fs.existsSync(sharedDeliveryReferenceSourceRoot)) {
    throw new Error(`Shared delivery references source does not exist: ${sharedDeliveryReferenceSourceRoot}`);
  }
  fs.rmSync(targetRoot, { recursive: true, force: true });
  copyDirectory(sharedDeliveryReferenceSourceRoot, targetRoot, sharedDeliveryReferenceSourceRoot);
}

function removeLegacyClaudeArtifacts() {
  const removed = [];
  for (const target of [
    path.join(commandsRoot, "loomline.md"),
    path.join(commandsRoot, "loomline-deploy.md"),
    legacyInstallRoot,
    legacyPluginDataPath,
  ]) {
    if (fs.existsSync(target)) {
      fs.rmSync(target, { recursive: true, force: true });
      removed.push(target);
    }
  }
  return removed;
}

function copyDirectory(source, target, root) {
  for (const entry of fs.readdirSync(source, { withFileTypes: true })) {
    const sourcePath = path.join(source, entry.name);
    if (entry.name === ".git" || entry.name === "node_modules") {
      continue;
    }
    const targetPath = path.join(target, path.relative(root, sourcePath));
    if (entry.isDirectory()) {
      fs.mkdirSync(targetPath, { recursive: true });
      copyDirectory(sourcePath, target, root);
    } else if (entry.isSymbolicLink()) {
      const real = fs.realpathSync(sourcePath);
      const stat = fs.statSync(real);
      if (stat.isDirectory()) {
        fs.mkdirSync(targetPath, { recursive: true });
        copyDirectory(real, targetPath, real);
      } else {
        fs.copyFileSync(real, targetPath);
      }
    } else if (entry.isFile()) {
      fs.mkdirSync(path.dirname(targetPath), { recursive: true });
      fs.copyFileSync(sourcePath, targetPath);
    }
  }
}

function assertExists(filePath) {
  if (!fs.existsSync(filePath)) {
    throw new Error(`Expected Claude plugin file was not copied: ${filePath}`);
  }
}
