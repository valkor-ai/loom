#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..");
const home = process.env.HOME || "";
const target = process.argv[2] || "all";
const validTargets = new Set(["codex", "claude", "opencode", "all"]);

if (!validTargets.has(target)) {
  throw new Error("Usage: npm run plugin:uninstall-<codex|claude|opencode|adapters>");
}

const adapters = target === "all" ? ["codex", "claude", "opencode"] : [target];
const removed = [];
const skipped = [];
const warnings = [];

for (const adapter of adapters) {
  uninstallAdapter(adapter);
}
cleanupSharedLauncher();

const result = {
  target,
  adapters,
  removed,
  skipped,
  warnings,
  preserved: [
    "Project-local .loom/ delivery state is not removed.",
    "The source repository and dist/ build output are not removed.",
  ],
  uninstalledAt: new Date().toISOString(),
};
console.log(JSON.stringify(result, null, 2));

function uninstallAdapter(adapter) {
  if (adapter === "codex") {
    uninstallCodex();
  } else if (adapter === "claude") {
    uninstallClaude();
  } else if (adapter === "opencode") {
    uninstallOpencode();
  }
  removeAdapterStamp(adapter);
}

function uninstallCodex() {
  const pluginName = "loom";
  const personalPluginRoot = path.join(home, "plugins", pluginName);
  const personalMarketplacePath = path.join(home, ".agents", "plugins", "marketplace.json");
  removeDirectoryIfLoomCodexSource(personalPluginRoot);
  removeCodexMarketplaceEntry(personalMarketplacePath, pluginName);
  removePath(path.join(home, ".codex", "plugins", "cache", "local-plugins", pluginName));
  removePath(path.join(home, ".codex", "plugins", "cache", "local", pluginName));
}

function uninstallClaude() {
  const claudeHome = process.env.CLAUDE_HOME || path.join(home, ".claude");
  const installRoot = path.join(claudeHome, "skills", "loom");
  const sourceCommandsRoot = path.join(repoRoot, "plugins", "claude-code", "commands");
  const commandsRoot = path.join(claudeHome, "commands");
  const stamp = readJson(path.join(installRoot, ".loom-claude-refresh.json"));
  const hasStamp = Boolean(stamp);
  const installedCommands = Array.isArray(stamp?.installedCommands)
    ? stamp.installedCommands
    : ["loom.md", "loom-deploy.md"].map((name) => path.join(commandsRoot, name));

  for (const commandPath of installedCommands) {
    const sourcePath = path.join(sourceCommandsRoot, path.basename(commandPath));
    removeFileIfGenerated(commandPath, sourcePath, hasStamp);
  }
  removeDirectoryIfLoomClaudeInstall(installRoot);
}

function uninstallOpencode() {
  const opencodeConfigRoot = process.env.OPENCODE_CONFIG_HOME || path.join(home, ".config", "opencode");
  const stampPath = path.join(opencodeConfigRoot, ".loom-opencode-refresh.json");
  const stamp = readJson(stampPath);
  const hasStamp = Boolean(stamp);
  const sourceRoot = path.join(repoRoot, "plugins", "opencode", ".opencode");

  const commandPaths = Array.isArray(stamp?.installedCommands)
    ? stamp.installedCommands
    : ["loom.md", "loom-deploy.md"].map((name) => path.join(opencodeConfigRoot, "commands", name));
  for (const commandPath of commandPaths) {
    removeFileIfGenerated(commandPath, path.join(sourceRoot, "commands", path.basename(commandPath)), hasStamp);
  }

  const pluginPaths = Array.isArray(stamp?.installedPlugins)
    ? stamp.installedPlugins
    : [path.join(opencodeConfigRoot, "plugins", "loom.js")];
  for (const pluginPath of pluginPaths) {
    removeFileIfGenerated(pluginPath, path.join(sourceRoot, "plugins", path.basename(pluginPath)), hasStamp);
  }

  const referencePaths = Array.isArray(stamp?.installedReferences)
    ? stamp.installedReferences
    : [path.join(opencodeConfigRoot, "references", "loom")];
  for (const referencePath of referencePaths) {
    removePath(referencePath);
  }
  removePath(stamp?.installedDeployReferencesRoot || path.join(opencodeConfigRoot, "loom-deploy", "references"));
  removeFile(stampPath);
}

function removeAdapterStamp(adapter) {
  const loomHome = process.env.LOOM_HOME || path.join(home, ".loom");
  removePath(path.join(loomHome, "adapters", adapter));
}

function cleanupSharedLauncher() {
  const loomHome = process.env.LOOM_HOME || path.join(home, ".loom");
  const adaptersRoot = path.join(loomHome, "adapters");
  const hasRemainingAdapters =
    fs.existsSync(adaptersRoot) &&
    fs.readdirSync(adaptersRoot).some((entry) => fs.existsSync(path.join(adaptersRoot, entry, "refresh.json")));
  if (!hasRemainingAdapters) {
    removeFile(path.join(loomHome, "bin", "loom-cli"));
    removeEmptyDir(path.join(loomHome, "bin"));
    removeEmptyDir(adaptersRoot);
    removeEmptyDir(loomHome);
  } else {
    skipped.push({
      path: path.join(loomHome, "bin", "loom-cli"),
      reason: "shared launcher kept because another Loom adapter is still installed",
    });
  }
}

function removeDirectoryIfLoomCodexSource(targetPath) {
  if (!fs.existsSync(targetPath)) {
    skipped.push({ path: targetPath, reason: "not found" });
    return;
  }
  const stampPath = path.join(targetPath, ".loom-codex-install-source.json");
  const manifestPath = path.join(targetPath, ".codex-plugin", "plugin.json");
  const manifest = readJson(manifestPath);
  if (!fs.existsSync(stampPath) && manifest?.name !== "loom") {
    skipped.push({ path: targetPath, reason: "not recognized as generated Loom Codex plugin source" });
    return;
  }
  removePath(targetPath);
}

function removeDirectoryIfLoomClaudeInstall(targetPath) {
  if (!fs.existsSync(targetPath)) {
    skipped.push({ path: targetPath, reason: "not found" });
    return;
  }
  const stampPath = path.join(targetPath, ".loom-claude-refresh.json");
  const manifest = readJson(path.join(targetPath, ".claude-plugin", "plugin.json"));
  if (!fs.existsSync(stampPath) && manifest?.name !== "loom") {
    skipped.push({ path: targetPath, reason: "not recognized as generated Loom Claude plugin install" });
    return;
  }
  removePath(targetPath);
}

function removeCodexMarketplaceEntry(filePath, pluginName) {
  const marketplace = readJson(filePath);
  if (!marketplace || !Array.isArray(marketplace.plugins)) {
    skipped.push({ path: filePath, reason: "marketplace not found or has no plugins array" });
    return;
  }
  const before = marketplace.plugins.length;
  marketplace.plugins = marketplace.plugins.filter((plugin) => {
    if (plugin?.name !== pluginName) return true;
    return plugin?.source?.source !== "local" || plugin?.source?.path !== "./plugins/loom";
  });
  if (marketplace.plugins.length === before) {
    skipped.push({ path: filePath, reason: "no generated Loom marketplace entry found" });
    return;
  }
  fs.writeFileSync(filePath, `${JSON.stringify(marketplace, null, 2)}\n`);
  removed.push({ path: filePath, action: "removed Loom marketplace entry" });
}

function removeFileIfGenerated(targetPath, sourcePath, trustedInstallPath = false) {
  if (!fs.existsSync(targetPath)) {
    skipped.push({ path: targetPath, reason: "not found" });
    return;
  }
  if (!trustedInstallPath && sourcePath && fs.existsSync(sourcePath)) {
    const target = fs.readFileSync(targetPath, "utf8");
    const source = fs.readFileSync(sourcePath, "utf8");
    if (target !== source) {
      skipped.push({ path: targetPath, reason: "file differs from Loom source; left untouched" });
      return;
    }
  }
  removeFile(targetPath);
}

function removePath(targetPath) {
  if (!targetPath || !fs.existsSync(targetPath)) {
    if (targetPath) skipped.push({ path: targetPath, reason: "not found" });
    return;
  }
  fs.rmSync(targetPath, { recursive: true, force: true });
  removed.push({ path: targetPath, action: "removed" });
}

function removeFile(targetPath) {
  if (!targetPath || !fs.existsSync(targetPath)) {
    if (targetPath) skipped.push({ path: targetPath, reason: "not found" });
    return;
  }
  fs.rmSync(targetPath, { force: true });
  removed.push({ path: targetPath, action: "removed" });
}

function removeEmptyDir(targetPath) {
  if (!targetPath || !fs.existsSync(targetPath)) {
    return;
  }
  const entries = fs.readdirSync(targetPath);
  if (entries.length > 0) {
    return;
  }
  fs.rmdirSync(targetPath);
  removed.push({ path: targetPath, action: "removed empty directory" });
}

function readJson(filePath) {
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch {
    return null;
  }
}
