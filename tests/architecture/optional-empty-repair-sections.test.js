#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "../..");

function issue(code, issuePath) {
  return {
    issueId: `issue-${code}-${issuePath}`,
    code,
    severity: "blocking",
    path: issuePath,
    message: "fixture issue",
    repairability: "agent_repairable",
    repairHint: "fixture repair hint",
  };
}

function main() {
  execFileSync("npm", ["run", "build"], { cwd: repoRoot, stdio: "inherit" });
  const { inferArchitectureRepairSections } = require(path.join(repoRoot, "dist/core/operations/contracts.js"));
  const { normalizeOptionalEmptyFields } = require(path.join(repoRoot, "dist/core/validators.js"));

  assert.deepEqual(inferArchitectureRepairSections([issue("UNKNOWN_ARTIFACT_REF", "/engineeringBoundary/modules/module-x")]), ["foundation"]);
  assert.deepEqual(inferArchitectureRepairSections([issue("UNKNOWN_ARTIFACT_REF", "/dataModel/entities/entity-x")]), ["domain_contract"]);
  assert.deepEqual(inferArchitectureRepairSections([issue("UNKNOWN_ARTIFACT_REF", "/interfaces/interface-x")]), ["domain_contract"]);
  assert.deepEqual(inferArchitectureRepairSections([issue("UNKNOWN_ARTIFACT_REF", "/userFlows/flow-x")]), ["behavior"]);
  assert.deepEqual(inferArchitectureRepairSections([issue("UNKNOWN_ARTIFACT_REF", "/stateMachines/sm-x")]), ["behavior"]);
  assert.deepEqual(inferArchitectureRepairSections([issue("SCHEMA_INVALID", "/frontendExperience/dataViews/0/viewId")]), ["frontend_experience"]);
  assert.deepEqual(inferArchitectureRepairSections([issue("SCHEMA_INVALID", "/runtimeDelivery/basis/previousRuntimeDeliveryRef")]), ["runtime_delivery"]);
  assert.deepEqual(inferArchitectureRepairSections([issue("AAC_COVERAGE_TYPE_MISMATCH", "/acceptanceMatrix/AC-001/coverage/data_constraint/rule-x")]), ["coverage"]);
  assert.deepEqual(inferArchitectureRepairSections([issue("SCHEMA_INVALID", "/detailCoverage/0/artifactRefs/modules")]), ["coverage"]);
  assert.deepEqual(inferArchitectureRepairSections([issue("SCHEMA_INVALID", "/detailCoverage/0/artifactRefs/interfaces")]), ["coverage"]);
  assert.deepEqual(inferArchitectureRepairSections([issue("SCHEMA_INVALID", "/detailCoverage/0/artifactRefs/userFlows")]), ["coverage"]);
  assert.deepEqual(inferArchitectureRepairSections([issue("SCHEMA_INVALID", "/detailCoverage/0/artifactRefs/stateMachines")]), ["coverage"]);
  assert.deepEqual(inferArchitectureRepairSections([issue("DETAIL_COVERAGE_INVALID", "/detailCoverage/detail-x/artifactRefs")]), ["coverage"]);
  assert.deepEqual(inferArchitectureRepairSections([issue("SCHEMA_INVALID", "/sections/behavior/content/userFlows/0/steps/0/action")]), ["behavior"]);
  assert.deepEqual(
    inferArchitectureRepairSections([
      issue("UNKNOWN_ARTIFACT_REF", "/modules/module-x"),
      issue("UNKNOWN_ARTIFACT_REF", "/acceptanceMatrix/AC-001/coverage/module/module-x"),
    ]),
    ["foundation", "coverage"],
  );

  const normalized = normalizeOptionalEmptyFields({
    requiredString: "",
    nested: {
      optionalText: "",
      optionalObject: null,
      keepNull: null,
      list: [{ optionalText: null, requiredString: "" }],
    },
  }, ["optionalText", "optionalObject"]);
  assert.equal(Object.hasOwn(normalized.nested, "optionalText"), false);
  assert.equal(Object.hasOwn(normalized.nested, "optionalObject"), false);
  assert.equal(Object.hasOwn(normalized.nested.list[0], "optionalText"), false);
  assert.equal(normalized.requiredString, "");
  assert.equal(normalized.nested.keepNull, null);
  assert.equal(normalized.nested.list[0].requiredString, "");

  console.log("optional empty normalization and AAC repair section mapping verification passed");
}

main();
