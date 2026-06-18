#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..");
const referenceNames = ["repair.md", "testing.md", "domain.md", "planning.md", "design.md", "review.md", "handoff.md"];
const adapterRoots = [
  {
    name: "codex",
    entrypoint: "plugins/codex/skills/loom/SKILL.md",
    referenceRoot: "plugins/codex/skills/loom/references/delivery",
    linkPrefix: "references/delivery",
  },
  {
    name: "claude",
    entrypoint: "plugins/claude-code/skills/loom/SKILL.md",
    referenceRoot: "plugins/claude-code/skills/loom/references/delivery",
    linkPrefix: "references/delivery",
  },
  {
    name: "opencode",
    entrypoint: "plugins/opencode/.opencode/commands/loom.md",
    referenceRoot: "plugins/opencode/.opencode/references/loom/delivery",
    linkPrefix: "../references/loom/delivery",
  },
];

const baselineRoot = adapterRoots[0].referenceRoot;
let failed = false;

for (const referenceName of referenceNames) {
  const baselinePath = path.join(repoRoot, baselineRoot, referenceName);
  const baseline = readRequired(baselinePath);
  for (const adapter of adapterRoots.slice(1)) {
    const candidatePath = path.join(repoRoot, adapter.referenceRoot, referenceName);
    const candidate = readRequired(candidatePath);
    if (candidate !== baseline) {
      failed = true;
      console.error(
        [
          `Delivery reference drift: ${referenceName}`,
          `  baseline: ${path.relative(repoRoot, baselinePath)}`,
          `  candidate: ${path.relative(repoRoot, candidatePath)}`,
        ].join("\n"),
      );
    }
  }
}

for (const adapter of adapterRoots) {
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
