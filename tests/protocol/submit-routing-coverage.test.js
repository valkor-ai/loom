#!/usr/bin/env node

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "../..");

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function assertContains(file, needle, message) {
  assert.ok(read(file).includes(needle), `${file}: ${message}`);
}

function main() {
  assertContains("src/core/operations/brainstorm.ts", "instruction: autoRunInstruction({", "Brainstorm accept success must return a direct instruction.");
  assertContains("src/core/operations/contracts.ts", "instruction: await technicalBaselineInstruction(root, locator, baseline)", "TechnicalBaseline accept success must return a direct instruction.");
  assertContains("src/core/operations/repository-context.ts", "instruction: autoRunInstruction({", "RepositoryContext accept success must return a direct instruction.");
  assertContains("src/core/operations/contracts.ts", "instruction: instructionForRouteAction({", "PlanningGenerationContract create success must return a direct instruction.");
  assertContains("src/core/operations/contracts.ts", "instruction: autoRunInstruction({", "Architecture accept success must return a direct instruction.");
  assertContains("src/core/operations/contracts.ts", "buildArchitectureRepairInstruction", "Architecture accept failure must return repairInstruction.");
  assertContains("src/core/operations/tasks.ts", "buildTaskPlanRepairInstruction", "TaskPlan accept failure must return repairInstruction.");
  assertContains("src/core/operations/tasks.ts", "actionType: \"continue_execution\"", "TaskPlan accept success must route to next task.");
  assertContains("src/core/operations/tasks.ts", "source: \"task_result_repair\"", "TaskResult repaired submit success must expose postRepairSubmitRouting.");
  assertContains("src/core/operations/review.ts", "instruction: instructionForRouteAction", "Review accept success must return a direct instruction.");
  assertContains("src/core/operations/review.ts", "instruction: instructionForRouteAction(routeAction", "Manual review resolve must return direct instruction for the selected route.");
  assertContains("src/core/operations/repair.ts", "function repairRequestGenerationInstruction", "RepairRequest creation must return a direct repair candidate generation instruction.");
  assertContains("src/core/operations/continue.ts", "repairSubmitCommandFromRequest", "continue recovery must resume active non-execution RepairRequest candidates.");
  assertContains("plugins/codex/skills/loom/SKILL.md", "After the repaired submit succeeds, immediately follow the successful response's `data.instruction`", "Skill must require repaired submit auto-routing.");
  console.log("submit routing coverage verification passed");
}

main();
