#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");

const repoRoot = path.resolve(__dirname, "../../../..");
const tsReferenceRoot = path.join(repoRoot, "src", "ts", "reference");
const deliveryNames = ["repair.md", "testing.md", "domain.md", "planning.md", "design.md", "review.md", "handoff.md"];

for (const root of [repoRoot, tsReferenceRoot]) {
  assertNoDeliveryReferences(root);
}

assertFileIncludes(path.join(repoRoot, "plugins", "shared", "loom", "references", "uix", "core.md"));
assertFileIncludes(path.join(repoRoot, "plugins", "shared", "loom-deploy", "references", "compose.md"));

console.log("Delivery references are not agent-facing; UIX and deploy references remain installed.");

function assertNoDeliveryReferences(root) {
  const sharedDeliveryRoot = path.join(root, "plugins", "shared", "loom", "references", "delivery");
  for (const name of deliveryNames) {
    assert.equal(
      fs.existsSync(path.join(sharedDeliveryRoot, name)),
      false,
      `${path.relative(repoRoot, path.join(sharedDeliveryRoot, name))} must not exist`,
    );
  }

  for (const file of [
    "plugins/codex/skills/loom/SKILL.md",
    "plugins/claude-code/skills/loom/SKILL.md",
  ]) {
    const content = readIfExists(path.join(root, file));
    assert.equal(
      content.includes("references/delivery/"),
      false,
      `${path.relative(repoRoot, path.join(root, file))} must not link delivery references`,
    );
  }

  for (const file of [
    "scripts/refresh-local-codex-plugin.js",
    "scripts/refresh-local-claude-plugin.js",
    "scripts/refresh-local-opencode-plugin.js",
  ]) {
    const content = readIfExists(path.join(root, file));
    assert.equal(
      content.includes("sharedDeliveryReferenceSourceRoot"),
      false,
      `${path.relative(repoRoot, path.join(root, file))} must not install delivery references`,
    );
    assert.equal(
      content.includes("deliveryReferenceSourceRoot"),
      false,
      `${path.relative(repoRoot, path.join(root, file))} must not install delivery references`,
    );
  }
}

function assertFileIncludes(filePath) {
  assert.equal(fs.existsSync(filePath), true, `${path.relative(repoRoot, filePath)} must exist`);
}

function readIfExists(filePath) {
  if (!fs.existsSync(filePath)) {
    return "";
  }
  return fs.readFileSync(filePath, "utf8");
}
