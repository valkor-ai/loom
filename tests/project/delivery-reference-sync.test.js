#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "../..");
const referenceNames = ["repair.md", "testing.md", "domain.md", "planning.md", "design.md", "review.md", "handoff.md"];
const sharedReferenceRoot = "plugins/shared/loom/references/delivery";
const adapterRoots = [
  {
    name: "codex",
    entrypoint: "plugins/codex/skills/loom/SKILL.md",
    localReferenceRoot: "plugins/codex/skills/loom/references/delivery",
    linkPrefix: "references/delivery",
  },
  {
    name: "claude",
    entrypoint: "plugins/claude-code/skills/loom/SKILL.md",
    localReferenceRoot: "plugins/claude-code/skills/loom/references/delivery",
    linkPrefix: "references/delivery",
  },
  {
    name: "opencode",
    entrypoint: "plugins/opencode/.opencode/commands/loom.md",
    localReferenceRoot: "plugins/opencode/.opencode/references/loom/delivery",
    linkPrefix: "../references/loom/delivery",
  },
];

let failed = false;

for (const referenceName of referenceNames) {
  readRequired(path.join(repoRoot, sharedReferenceRoot, referenceName));
}

for (const adapter of adapterRoots) {
  const localReferencePath = path.join(repoRoot, adapter.localReferenceRoot);
  if (fs.existsSync(localReferencePath)) {
    failed = true;
    console.error(
      `${adapter.localReferenceRoot}: delivery references must be installed from ${sharedReferenceRoot}, not maintained as adapter-local copies`,
    );
  }

  const entrypointPath = path.join(repoRoot, adapter.entrypoint);
  const entrypoint = readRequired(entrypointPath);
  for (const referenceName of referenceNames) {
    const expectedLink = `${adapter.linkPrefix}/${referenceName}`;
    if (!entrypoint.includes(expectedLink)) {
      failed = true;
      console.error(
        `Missing ${adapter.name} delivery reference link in ${adapter.entrypoint}: ${expectedLink}`,
      );
    }
  }
}

for (const expectation of [
  {
    file: "scripts/refresh-local-codex-plugin.js",
    snippets: [
      "sharedDeliveryReferenceSourceRoot",
      "\"plugins\", \"shared\", \"loom\", \"references\", \"delivery\"",
      "\"skills\", \"loom\", \"references\", \"delivery\"",
    ],
  },
  {
    file: "scripts/refresh-local-claude-plugin.js",
    snippets: [
      "sharedDeliveryReferenceSourceRoot",
      "\"plugins\", \"shared\", \"loom\", \"references\", \"delivery\"",
      "\"skills\", \"loom\", \"references\", \"delivery\"",
    ],
  },
  {
    file: "scripts/refresh-local-opencode-plugin.js",
    snippets: [
      "deliveryReferenceSourceRoot",
      "\"plugins\", \"shared\", \"loom\", \"references\", \"delivery\"",
      "\"loom\", \"delivery\"",
    ],
  },
]) {
  const script = readRequired(path.join(repoRoot, expectation.file));
  for (const snippet of expectation.snippets) {
    if (!script.includes(snippet)) {
      failed = true;
      console.error(`${expectation.file}: missing delivery reference install snippet: ${snippet}`);
    }
  }
}

if (failed) {
  process.exit(1);
}

console.log("Delivery references are synchronized across adapters.");

function readRequired(filePath) {
  if (!fs.existsSync(filePath)) {
    failed = true;
    console.error(`Missing required delivery reference: ${path.relative(repoRoot, filePath)}`);
    return "";
  }
  return fs.readFileSync(filePath, "utf8");
}
