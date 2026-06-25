#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "../../../../src/ts/reference");

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function assertIncludes(source, needle, message) {
  assert.ok(source.includes(needle), message);
}

function sourceNotReadyLiteralPaths(validatorSource) {
  const paths = [];
  const pattern = /issue\("SOURCE_NOT_READY",\s*"([^"`]+?)"/g;
  for (const match of validatorSource.matchAll(pattern)) {
    paths.push(match[1]);
  }
  return paths;
}

function issue(issuePath) {
  return {
    issueId: `issue-${issuePath}`,
    code: "SOURCE_NOT_READY",
    severity: "blocking",
    path: issuePath,
    message: "fixture",
    repairability: "blocked",
    repairHint: "fixture",
  };
}

function main() {
  execFileSync("npm", ["run", "build"], { cwd: repoRoot, stdio: "inherit" });

  const validatorSource = read("core/validators.ts");
  const contractsSource = read("core/operations/contracts.ts");
  const guideSource = read("core/operations/artifact-read-guide.ts");
  const { inferArchitectureRepairSections } = require(path.join(repoRoot, "dist/core/operations/contracts.js"));

  const sourceNotReadyPaths = sourceNotReadyLiteralPaths(validatorSource);
  const architectureBoundaryPaths = sourceNotReadyPaths.filter((issuePath) =>
    issuePath.startsWith("/frontendExperience") || issuePath.startsWith("/runtimeDelivery")
  );
  assert.ok(
    architectureBoundaryPaths.length > 0,
    "protocol audit must cover literal AAC SOURCE_NOT_READY paths from validators",
  );

  for (const issuePath of architectureBoundaryPaths) {
    const expectedSection = issuePath.startsWith("/runtimeDelivery") ? "runtime_delivery" : "frontend_experience";
    assert.deepEqual(
      inferArchitectureRepairSections([issue(issuePath)]),
      [expectedSection],
      `${issuePath} must route repair to ${expectedSection}`,
    );
  }

  const requiredProtocolSnippets = [
    {
      file: "contracts.ts",
      source: contractsSource,
      needle: "technicalBaselineRef: toProjectRelative(root, technicalBaselinePath(root, locator.deliveryId))",
      message: "ArchitectureRequest must expose technicalBaselineRef for runtimeDelivery source validation.",
    },
    {
      file: "contracts.ts",
      source: contractsSource,
      needle: "planningContractRef: toProjectRelative(root, planningContractPath(root, pgc.planningContractId, locator))",
      message: "ArchitectureRequest must expose planningContractRef for runtimeDelivery source validation.",
    },
    {
      file: "contracts.ts",
      source: contractsSource,
      needle: "...(previousRuntimeRef ? { previousRuntimeDeliveryRef: previousRuntimeRef } : {})",
      message: "ArchitectureRequest must expose previousRuntimeDeliveryRef when a completed prior phase exists.",
    },
    {
      file: "contracts.ts",
      source: contractsSource,
      needle: "sourceRefs.previousRuntimeDeliveryRef",
      message: "Agent-facing ArchitectureRequest rules must name previousRuntimeDeliveryRef as a sourceRefs authority.",
    },
    {
      file: "artifact-read-guide.ts",
      source: guideSource,
      needle: "previousRuntimeDeliveryRef: runtimeDeliveryGuide",
      message: "referencedArtifactReadGuide must explain previousRuntimeDeliveryRef.",
    },
    {
      file: "contracts.ts",
      source: contractsSource,
      needle: "frontendExperienceSource",
      message: "ArchitectureRequest must expose frontendExperienceSource for frontendExperience source validation.",
    },
    {
      file: "contracts.ts",
      source: contractsSource,
      needle: "brainstormFrontendExperienceRef",
      message: "ArchitectureRequest rules must explain frontendExperience source ref selection.",
    },
    {
      file: "contracts.ts",
      source: contractsSource,
      needle: "If sourceRefs.previousRuntimeDeliveryRef is absent, do not wait for another source contract and do not keep runtimeDelivery.status=unchanged",
      message: "Repair instruction must not tell agents to wait for a missing previousRuntimeDeliveryRef in old requests.",
    },
  ];

  for (const item of requiredProtocolSnippets) {
    assertIncludes(item.source, item.needle, `${item.file}: ${item.message}`);
  }

  assertIncludes(
    contractsSource,
    "return \"Read request.sourceRefs.previousRuntimeDeliveryRef.",
    "runtimeDelivery previous ref repair hint must override the generic source-not-ready hint.",
  );

  console.log("protocol boundary audit verification passed");
}

main();
