#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const repoRoot = path.resolve(__dirname, "..");
const testRoot = __dirname;
const suiteArg = process.argv[2] ?? "all";
const filters = process.argv.slice(3);
const suites = [
  "deploy",
  "brainstorm",
  "architecture",
  "task",
  "knowledge",
  "protocol",
  "adapters",
  "review",
  "project",
];

if (suiteArg === "-h" || suiteArg === "--help") {
  usage(0);
}

if (suiteArg !== "all" && !suites.includes(suiteArg)) {
  console.error(`Unknown test suite: ${suiteArg}`);
  usage(1);
}

const selectedSuites = suiteArg === "all" ? suites : [suiteArg];
const files = selectedSuites.flatMap((suite) => suiteFiles(suite, filters));

if (files.length === 0) {
  console.error(`No test files matched suite=${suiteArg} filters=${filters.join(",") || "<none>"}.`);
  process.exit(1);
}

for (const file of files) {
  const relative = path.relative(repoRoot, file);
  console.log(`\n[test] ${relative}`);
  const result = spawnSync(process.execPath, [file], {
    cwd: repoRoot,
    stdio: "inherit",
    env: process.env,
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

console.log(`\n${files.length} test file(s) passed.`);

function suiteFiles(suite, activeFilters) {
  const suiteDir = path.join(testRoot, suite);
  const files = fs
    .readdirSync(suiteDir)
    .filter((name) => name.endsWith(".test.js"))
    .sort()
    .map((name) => path.join(suiteDir, name));
  if (activeFilters.length === 0) {
    return files;
  }
  return files.filter((file) => {
    const name = path.basename(file, ".test.js");
    return activeFilters.some((filter) => name === filter || name.includes(filter));
  });
}

function usage(exitCode) {
  const text = [
    "Usage: node tests/run-suite.js <all|suite> [name-filter...]",
    "",
    `Suites: ${suites.join(", ")}`,
    "",
    "Examples:",
    "  node tests/run-suite.js deploy",
    "  node tests/run-suite.js knowledge registration",
  ].join("\n");
  (exitCode === 0 ? console.log : console.error)(text);
  process.exit(exitCode);
}
