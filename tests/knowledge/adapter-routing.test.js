#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "../..");
let failed = false;

const files = {
  codex: "plugins/codex/skills/loom/SKILL.md",
  claudeSkill: "plugins/claude-code/skills/loom/SKILL.md",
  claudeCommand: "plugins/claude-code/commands/loom.md",
  opencode: "plugins/opencode/.opencode/commands/loom.md",
};

const contents = Object.fromEntries(
  Object.entries(files).map(([name, relativePath]) => [name, readRequired(relativePath)]),
);

expect(
  "codex",
  "@loom knowledge <subcommand>",
  "Codex adapter must expose @loom knowledge as a first-class direct command.",
);
expect(
  "codex",
  'LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" knowledge <subcommand and args> --project-root /abs/project',
  "Codex adapter must route knowledge subcommands directly through the user launcher.",
);
expect(
  "codex",
  "They are not delivery requests",
  "Codex adapter must forbid treating knowledge commands as delivery requests.",
);
expect(
  "codex",
  "do not run `plan`, `continue`, Brainstorm, candidate generation, task execution, or deploy routing before the knowledge command",
  "Codex adapter must prevent knowledge commands from falling through to Brainstorm or delivery routing.",
);
expect(
  "codex",
  "`generate_knowledge_semantics`",
  "Codex adapter must know how to complete semantic build packs returned by knowledge build.",
);
expect(
  "codex",
  "request.outputContract.resultTemplate",
  "Codex adapter must use the semantic result template instead of guessing the schema.",
);
expect(
  "codex",
  "Do not inspect Loom source files",
  "Codex adapter must forbid reading Loom source to infer semantic result schema.",
);
expect(
  "codex",
  "build, resume",
  "Codex adapter must mention knowledge resume as a direct knowledge command.",
);

expect(
  "claudeSkill",
  'argument-hint: "<request> | plan <request> | continue | knowledge [subcommand] | deploy [subcommand] | status"',
  "Claude skill must advertise knowledge in the slash-command hint.",
);
expect(
  "claudeSkill",
  "/loom knowledge <subcommand>",
  "Claude skill must expose /loom knowledge as a first-class direct command.",
);
expect(
  "claudeSkill",
  'LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" knowledge <subcommand and args> --project-root /abs/project',
  "Claude skill must route knowledge subcommands directly through the user launcher.",
);
expect(
  "claudeSkill",
  "For `/loom knowledge ...`, do not run `plan`, `continue`, Brainstorm, candidate generation, task execution, or deploy routing before the knowledge command",
  "Claude skill must prevent knowledge commands from falling through to Brainstorm or delivery routing.",
);
expect(
  "claudeSkill",
  "`generate_knowledge_semantics`",
  "Claude skill must know how to complete semantic build packs returned by knowledge build.",
);
expect(
  "claudeSkill",
  "request.outputContract.resultTemplate",
  "Claude skill must use the semantic result template instead of guessing the schema.",
);
expect(
  "claudeSkill",
  "Do not inspect Loom source files",
  "Claude skill must forbid reading Loom source to infer semantic result schema.",
);
expect(
  "claudeSkill",
  "`knowledge build` and `knowledge resume` may return `generate_knowledge_semantics`",
  "Claude skill must treat knowledge resume as semantic build recovery.",
);

expect(
  "claudeCommand",
  'argument-hint: "<request> | plan <request> | continue | knowledge [subcommand] | deploy [subcommand] | status"',
  "Claude command wrapper must advertise knowledge in the slash-command hint.",
);
expect(
  "claudeCommand",
  "If `$ARGUMENTS` starts with `knowledge `",
  "Claude command wrapper must dispatch knowledge before new delivery planning.",
);
expect(
  "claudeCommand",
  'LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" knowledge <the knowledge subcommand arguments from $ARGUMENTS> --project-root "$PWD"',
  "Claude command wrapper must run knowledge subcommands directly.",
);
expect(
  "claudeCommand",
  "For knowledge routes, parse the returned JSON envelope",
  "Claude command wrapper must keep knowledge responses out of delivery protocol loading.",
);
expect(
  "claudeCommand",
  "`knowledge build` and `knowledge resume` may return `generate_knowledge_semantics`",
  "Claude command wrapper must treat knowledge resume as semantic build recovery.",
);
expect(
  "claudeCommand",
  "request.outputContract.resultTemplate",
  "Claude command wrapper must use the semantic result template instead of guessing the schema.",
);
expect(
  "claudeCommand",
  "do not inspect Loom source files",
  "Claude command wrapper must forbid reading Loom source to infer semantic result schema.",
);
expect(
  "opencode",
  'argument-hint: "<request> | plan <request> | continue | knowledge [subcommand] | deploy [subcommand] | status"',
  "OpenCode command must advertise knowledge in the slash-command hint.",
);
expect(
  "opencode",
  "If `First token` is exactly `knowledge`, this is an explicit knowledge-source command and this command file must handle it directly",
  "OpenCode command must dispatch knowledge before delivery-state routing.",
);
expect(
  "opencode",
  'If `First token = knowledge` and `Second token` is non-empty, run exactly `LOOM_AGENT_PROFILE=opencode LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" knowledge <Second token and remaining knowledge arguments> --project-root /abs/project`',
  "OpenCode command must run knowledge subcommands directly.",
);
expect(
  "opencode",
  "do not route into phase planning, Brainstorm, candidate generation, task execution, review, repair, or deploy",
  "OpenCode command must keep knowledge commands out of delivery routing.",
);
expect(
  "opencode",
  "`generate_knowledge_semantics`",
  "OpenCode command must know how to complete semantic build packs returned by knowledge build.",
);
expect(
  "opencode",
  "request.outputContract.resultTemplate",
  "OpenCode command must use the semantic result template instead of guessing the schema.",
);
expect(
  "opencode",
  "Do not inspect Loom source files",
  "OpenCode command must forbid reading Loom source to infer semantic result schema.",
);
expect(
  "opencode",
  "Build/resume may return `generate_knowledge_semantics`",
  "OpenCode command must treat knowledge resume as semantic build recovery.",
);

forbidInAdapters(
  "script-generated label/affinity factories",
  "Adapter routing files must not carry semantic field-generation rules; request/instruction contracts own them.",
);
forbidInAdapters(
  "Decide summary, semantic labels",
  "Adapter routing files must not decide semantic fields.",
);
forbidInAdapters(
  "Use chunk meaning for summary/labels/affinity",
  "Adapter routing files must not carry chunk semantic-generation guidance.",
);

assertOrder(
  "claudeCommand",
  "If `$ARGUMENTS` starts with `knowledge `",
  "Otherwise, treat `$ARGUMENTS` as a new Loom delivery request",
  "Claude command wrapper must check knowledge before falling through to plan.",
);
assertOrder(
  "opencode",
  "If `First token` is exactly `knowledge`",
  "If `First token` is exactly `deploy`",
  "OpenCode command must check knowledge before other delivery-state routing.",
);
assertOrder(
  "opencode",
  "If `First token` is exactly `knowledge`",
  "<request>` or `plan <request>`",
  "OpenCode command must check knowledge before new delivery planning.",
);

if (failed) {
  process.exit(1);
}

console.log("Knowledge adapter routing verification passed.");

function readRequired(relativePath) {
  const fullPath = path.join(repoRoot, relativePath);
  if (!fs.existsSync(fullPath)) {
    failed = true;
    console.error(`Missing required adapter file: ${relativePath}`);
    return "";
  }
  return fs.readFileSync(fullPath, "utf8");
}

function expect(fileKey, snippet, message) {
  if (!contents[fileKey].includes(snippet)) {
    failed = true;
    console.error(`${files[fileKey]}: ${message}`);
    console.error(`Missing snippet: ${snippet}`);
  }
}

function forbidInAdapters(snippet, message) {
  for (const [fileKey, content] of Object.entries(contents)) {
    if (content.includes(snippet)) {
      failed = true;
      console.error(`${files[fileKey]}: ${message}`);
      console.error(`Forbidden snippet: ${snippet}`);
    }
  }
}

function assertOrder(fileKey, firstSnippet, secondSnippet, message) {
  const content = contents[fileKey];
  const firstIndex = content.indexOf(firstSnippet);
  const secondIndex = content.indexOf(secondSnippet);
  if (firstIndex === -1 || secondIndex === -1 || firstIndex > secondIndex) {
    failed = true;
    console.error(`${files[fileKey]}: ${message}`);
  }
}
