#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const { ensureLoomUserInstall } = require("./lib/loom-user-install");

const repoRoot = path.resolve(__dirname, "..");
const codexPluginRoot = path.join(repoRoot, "plugins", "codex");
const sharedDeployReferenceSourceRoot = path.join(repoRoot, "plugins", "shared", "loom-deploy", "references");
const manifestPath = path.join(codexPluginRoot, ".codex-plugin", "plugin.json");
const sourceManifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const pluginName = sourceManifest.name;
const sourceVersion = sourceManifest.version;
const home = process.env.HOME || "";
const personalMarketplaceRoot = path.join(home, ".agents", "plugins");
const personalMarketplacePath = path.join(personalMarketplaceRoot, "marketplace.json");
const personalPluginRoot = path.join(home, "plugins", pluginName);
const legacyPluginName = "loomline";
const legacyPersonalPluginRoot = path.join(home, "plugins", legacyPluginName);
const legacyLocalPluginCacheRoot = path.join(home, ".codex", "plugins", "cache", "local", legacyPluginName);
const legacyLocalMarketplaceCacheRoot = path.join(home, ".codex", "plugins", "cache", "local-plugins", legacyPluginName);
const cliTarget = path.join(repoRoot, "dist", "cli.js");
const skipCodexAdd = process.env.LOOM_SKIP_CODEX_PLUGIN_ADD === "1";

if (!pluginName || !sourceVersion) {
  throw new Error("plugins/codex/.codex-plugin/plugin.json must contain name and version.");
}
if (!fs.existsSync(cliTarget)) {
  throw new Error("dist/cli.js does not exist. Run npm run build first.");
}

const removedLegacyArtifacts = removeLegacyCodexArtifacts();
const installVersion = withCodexCachebuster(sourceVersion, process.env.LOOM_CODEX_CACHEBUSTER || localCachebuster());
installCodexSource();
const marketplace = ensurePersonalMarketplaceEntry();

const userInstall = ensureLoomUserInstall({
  adapter: "codex",
  repoRoot,
  pluginInstallRoot: personalPluginRoot,
});

let codexInstall = {
  skipped: skipCodexAdd,
  command: ["codex", "plugin", "add", `${pluginName}@${marketplace.name}`],
};
if (!skipCodexAdd) {
  codexInstall = {
    ...codexInstall,
    ...runCodexPluginAdd(marketplace.name),
  };
}

const stamp = {
  pluginName,
  sourceVersion,
  installVersion,
  source: codexPluginRoot,
  installSourceRoot: personalPluginRoot,
  marketplacePath: personalMarketplacePath,
  marketplaceName: marketplace.name,
  marketplaceSourcePath: "./plugins/loom",
  removedLegacyArtifactCount: removedLegacyArtifacts.length,
  cli: {
    cliTarget,
    launcherPath: userInstall.launcherPath,
    launcherRef: userInstall.launcherRef,
    userInstall,
  },
  codexInstall,
  refreshedAt: new Date().toISOString(),
};
fs.writeFileSync(path.join(personalPluginRoot, ".loom-codex-install-source.json"), `${JSON.stringify(stamp, null, 2)}\n`);
fs.writeFileSync(path.join(userInstall.adapterRoot, "refresh.json"), `${JSON.stringify(stamp, null, 2)}\n`);
console.log(JSON.stringify(stamp, null, 2));

function installCodexSource() {
  prepareInstallRoot(personalPluginRoot);
  copyDirectory(codexPluginRoot, personalPluginRoot);
  installSharedDeployReferences(path.join(personalPluginRoot, "skills", "loom-deploy", "references"));
  fs.rmSync(path.join(personalPluginRoot, "skills", "loom", "references", "delivery"), { recursive: true, force: true });

  const installedManifestPath = path.join(personalPluginRoot, ".codex-plugin", "plugin.json");
  const installedManifest = JSON.parse(fs.readFileSync(installedManifestPath, "utf8"));
  installedManifest.version = installVersion;
  fs.writeFileSync(installedManifestPath, `${JSON.stringify(installedManifest, null, 2)}\n`);

  assertExists(path.join(personalPluginRoot, ".codex-plugin", "plugin.json"));
  assertExists(path.join(personalPluginRoot, "skills", "loom", "SKILL.md"));
  assertExists(path.join(personalPluginRoot, "skills", "loom-deploy", "SKILL.md"));
  assertExists(path.join(personalPluginRoot, "skills", "loom-deploy", "references", "node.md"));
}

function installSharedDeployReferences(targetRoot) {
  if (!fs.existsSync(sharedDeployReferenceSourceRoot)) {
    throw new Error(`Shared deploy references source does not exist: ${sharedDeployReferenceSourceRoot}`);
  }
  fs.rmSync(targetRoot, { recursive: true, force: true });
  copyDirectory(sharedDeployReferenceSourceRoot, targetRoot);
}

function prepareInstallRoot(target) {
  if (!fs.existsSync(target)) {
    fs.mkdirSync(path.dirname(target), { recursive: true });
    return;
  }

  const stat = fs.lstatSync(target);
  if (stat.isSymbolicLink()) {
    fs.rmSync(target);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    return;
  }
  if (!stat.isDirectory()) {
    throw new Error(`Cannot replace Codex plugin install source because it is not a directory: ${target}`);
  }

  const existingManifestPath = path.join(target, ".codex-plugin", "plugin.json");
  const existingStampPath = path.join(target, ".loom-codex-install-source.json");
  const entries = fs.readdirSync(target);
  const isEmpty = entries.length === 0;
  const isLoomPlugin =
    fs.existsSync(existingManifestPath) &&
    JSON.parse(fs.readFileSync(existingManifestPath, "utf8")).name === pluginName;
  const isGeneratedInstall = fs.existsSync(existingStampPath);
  if (!isEmpty && !isLoomPlugin && !isGeneratedInstall) {
    throw new Error(`Refusing to replace non-Loom Codex plugin source: ${target}`);
  }

  fs.rmSync(target, { recursive: true, force: true });
  fs.mkdirSync(path.dirname(target), { recursive: true });
}

function ensurePersonalMarketplaceEntry() {
  fs.mkdirSync(personalMarketplaceRoot, { recursive: true });
  const marketplace = fs.existsSync(personalMarketplacePath)
    ? JSON.parse(fs.readFileSync(personalMarketplacePath, "utf8"))
    : {
        name: "local-plugins",
        interface: { displayName: "Local Plugins" },
        plugins: [],
      };

  if (!marketplace.name) {
    marketplace.name = "local-plugins";
  }
  if (!marketplace.interface) {
    marketplace.interface = { displayName: "Local Plugins" };
  }
  if (!Array.isArray(marketplace.plugins)) {
    marketplace.plugins = [];
  }
  marketplace.plugins = marketplace.plugins.filter((plugin) => plugin?.name !== legacyPluginName);

  const entry = {
    name: pluginName,
    source: {
      source: "local",
      path: "./plugins/loom",
    },
    policy: {
      installation: "AVAILABLE",
      authentication: "ON_INSTALL",
    },
    category: "Productivity",
  };

  const index = marketplace.plugins.findIndex((plugin) => plugin?.name === pluginName);
  if (index >= 0) {
    marketplace.plugins[index] = entry;
  } else {
    marketplace.plugins.push(entry);
  }

  fs.writeFileSync(personalMarketplacePath, `${JSON.stringify(marketplace, null, 2)}\n`);
  return marketplace;
}

function removeLegacyCodexArtifacts() {
  const removed = [];
  for (const target of [
    legacyPersonalPluginRoot,
    legacyLocalPluginCacheRoot,
    legacyLocalMarketplaceCacheRoot,
  ]) {
    if (target && fs.existsSync(target)) {
      fs.rmSync(target, { recursive: true, force: true });
      removed.push(target);
    }
  }
  return removed;
}

function runCodexPluginAdd(marketplaceName) {
  const selector = `${pluginName}@${marketplaceName}`;
  const result = spawnSync("codex", ["plugin", "add", selector], {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.error) {
    throw new Error(`Failed to run codex plugin add. Make sure Codex CLI is installed: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(
      [
        `codex plugin add ${selector} failed with exit code ${result.status}.`,
        result.stdout ? `stdout:\n${result.stdout.trim()}` : null,
        result.stderr ? `stderr:\n${result.stderr.trim()}` : null,
      ]
        .filter(Boolean)
        .join("\n"),
    );
  }
  return {
    skipped: false,
    status: result.status,
    stdout: result.stdout.trim(),
    stderr: result.stderr.trim(),
  };
}

function copyDirectory(source, target) {
  fs.mkdirSync(target, { recursive: true });
  for (const entry of fs.readdirSync(source, { withFileTypes: true })) {
    const sourcePath = path.join(source, entry.name);
    const targetPath = path.join(target, entry.name);
    if (entry.isDirectory()) {
      copyDirectory(sourcePath, targetPath);
    } else if (entry.isSymbolicLink()) {
      const real = fs.realpathSync(sourcePath);
      const stat = fs.statSync(real);
      if (stat.isDirectory()) {
        copyDirectory(real, targetPath);
      } else {
        fs.copyFileSync(real, targetPath);
      }
    } else if (entry.isFile()) {
      fs.copyFileSync(sourcePath, targetPath);
    }
  }
}

function withCodexCachebuster(version, cachebuster) {
  const baseVersion = String(version).split("+")[0];
  return `${baseVersion}+codex.${cachebuster}`;
}

function localCachebuster() {
  const now = new Date();
  const yyyy = now.getUTCFullYear();
  const mm = String(now.getUTCMonth() + 1).padStart(2, "0");
  const dd = String(now.getUTCDate()).padStart(2, "0");
  const hh = String(now.getUTCHours()).padStart(2, "0");
  const mi = String(now.getUTCMinutes()).padStart(2, "0");
  const ss = String(now.getUTCSeconds()).padStart(2, "0");
  return `local-${yyyy}${mm}${dd}-${hh}${mi}${ss}`;
}

function assertExists(filePath) {
  if (!fs.existsSync(filePath)) {
    throw new Error(`Expected Codex plugin install file was not copied: ${filePath}`);
  }
}
