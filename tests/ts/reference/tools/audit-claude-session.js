#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const args = parseArgs(process.argv.slice(2));
const projectRoot = path.resolve(args.projectRoot || process.cwd());
const claudeProjectDir = args.logDir || path.join(homeDir(), ".claude", "projects", encodeClaudeProjectDir(projectRoot));
const sessionFile = resolveSessionFile(claudeProjectDir, args.session);
const events = readJsonl(sessionFile);
const findings = audit(events);

if (args.json) {
  console.log(JSON.stringify({ projectRoot, sessionFile, ...findings }, null, 2));
} else {
  printReport(projectRoot, sessionFile, findings);
}

function parseArgs(argv) {
  const out = { projectRoot: null, session: "latest", logDir: null, json: false };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--project-root") out.projectRoot = argv[++i];
    else if (arg === "--session") out.session = argv[++i];
    else if (arg === "--log-dir") out.logDir = argv[++i];
    else if (arg === "--json") out.json = true;
    else if (arg === "-h" || arg === "--help") usage(0);
    else {
      console.error(`Unknown argument: ${arg}`);
      usage(1);
    }
  }
  return out;
}

function usage(exitCode) {
  const text = [
    "Usage: node tests/tools/audit-claude-session.js --project-root /abs/project [--session latest|<session-id>] [--json]",
    "",
    "Audits Claude Code JSONL session logs for Loom adapter boundary issues without running a delivery.",
  ].join("\n");
  (exitCode === 0 ? console.log : console.error)(text);
  process.exit(exitCode);
}

function homeDir() {
  if (!process.env.HOME) {
    throw new Error("HOME is required to locate Claude logs.");
  }
  return process.env.HOME;
}

function encodeClaudeProjectDir(root) {
  return path.resolve(root).replace(/\//g, "-");
}

function resolveSessionFile(logDir, session) {
  if (!fs.existsSync(logDir)) {
    throw new Error(`Claude log directory does not exist: ${logDir}`);
  }
  if (session && session !== "latest") {
    const direct = path.join(logDir, session.endsWith(".jsonl") ? session : `${session}.jsonl`);
    if (!fs.existsSync(direct)) {
      throw new Error(`Claude session log does not exist: ${direct}`);
    }
    return direct;
  }

  const files = fs
    .readdirSync(logDir)
    .filter((name) => name.endsWith(".jsonl"))
    .map((name) => {
      const file = path.join(logDir, name);
      return { file, mtimeMs: fs.statSync(file).mtimeMs };
    })
    .sort((a, b) => b.mtimeMs - a.mtimeMs);
  if (!files.length) {
    throw new Error(`No Claude session logs found in ${logDir}`);
  }
  return files[0].file;
}

function readJsonl(file) {
  return fs
    .readFileSync(file, "utf8")
    .split(/\n/)
    .map((line, index) => ({ line, lineNo: index + 1 }))
    .filter((entry) => entry.line.trim())
    .map((entry) => {
      try {
        return { lineNo: entry.lineNo, value: JSON.parse(entry.line) };
      } catch (error) {
        return { lineNo: entry.lineNo, parseError: error.message };
      }
    });
}

function audit(events) {
  const toolUses = [];
  const hookErrors = [];
  const stopGuardBlocks = [];
  const malformedReads = [];
  const malformedWrites = [];
  const largeOutputs = [];
  const fullArtifactReads = [];

  for (const event of events) {
    const value = event.value;
    if (!value) continue;

    const text = eventText(value);
    if (value.type === "attachment" && String(value.attachment?.type || "").includes("hook")) {
      const hookName = value.attachment?.hookName || "";
      const stderr = String(value.attachment?.stderr || "");
      const stdout = String(value.attachment?.stdout || "");
      const item = {
        line: event.lineNo,
        timestamp: value.timestamp || null,
        type: value.attachment?.type || null,
        hookName,
        message: firstNonEmpty(stderr, stdout, value.attachment?.content || ""),
      };
      hookErrors.push(item);
      if (hookName === "Stop" && String(value.attachment?.type || "").includes("blocking")) {
        stopGuardBlocks.push(item);
      }
    }
    if (/Hook JSON output validation failed|hookSpecificOutput is missing/.test(text)) {
      hookErrors.push({
        line: event.lineNo,
        timestamp: value.timestamp || null,
        type: "hook_schema_validation",
        hookName: value.attachment?.hookName || null,
        message: text.match(/Hook JSON output validation failed[^\n]*/)?.[0] || "Hook JSON output validation failed",
      });
    }
    if (/InputValidationError: Read failed|pages.+empty|file_path.+missing|file_path.+undefined/i.test(text)) {
      malformedReads.push({ line: event.lineNo, timestamp: value.timestamp || null, message: snippet(text) });
    }
    if (/InputValidationError: Write failed|The required parameter `file_path` is missing|The required parameter `content` is missing/.test(text)) {
      malformedWrites.push({ line: event.lineNo, timestamp: value.timestamp || null, message: snippet(text) });
    }

    const content = value.message?.content;
    if (Array.isArray(content)) {
      for (const item of content) {
        if (item?.type !== "tool_use") continue;
        toolUses.push({ line: event.lineNo, timestamp: value.timestamp || null, name: item.name, input: item.input || {} });
      }
    }

    if (value.type === "user") {
      const serialized = JSON.stringify(value.message?.content || value.toolUseResult || "");
      if (serialized.length > 20000) {
        largeOutputs.push({ line: event.lineNo, timestamp: value.timestamp || null, bytes: serialized.length, excerpt: snippet(text) });
      }
    }
  }

  for (const use of toolUses) {
    if (use.name !== "Read") continue;
    const filePath = String(use.input.file_path || "");
    if (filePath.includes(".loom/") && !filePath.includes("/execution-requests/")) {
      fullArtifactReads.push({ line: use.line, timestamp: use.timestamp, filePath });
    }
    if (use.input.pages === "" || (Array.isArray(use.input.pages) && use.input.pages.length === 0) || use.input.file_path === "") {
      malformedReads.push({ line: use.line, timestamp: use.timestamp, message: `Malformed Read input: ${JSON.stringify(use.input)}` });
    }
  }

  const bashCommands = toolUses
    .filter((use) => use.name === "Bash")
    .map((use) => ({ ...use, command: String(use.input.command || "") }));
  const loomCommands = bashCommands.filter((use) => /\bloom(?:-cli)?\b/.test(use.command));
  const bareLoom = loomCommands.filter((use) => /(^|[\s;&|])loom(\s|$)/.test(use.command) && !use.command.includes("loom-cli"));
  const missingClaudeProfile = loomCommands.filter((use) => use.command.includes("loom-cli") && !use.command.includes("LOOM_AGENT_PROFILE=claude"));
  const missingCompact = loomCommands.filter((use) => use.command.includes("loom-cli") && !use.command.includes("LOOM_COMPACT_OUTPUT=1"));
  const forbiddenPlanModeTools = toolUses.filter((use) =>
    ["EnterPlanMode", "ExitPlanMode"].includes(use.name),
  );
  const internalImplementationAids = toolUses.filter((use) =>
    ["TaskCreate", "TaskUpdate", "TaskList", "TaskGet", "TaskOutput", "Agent"].includes(use.name),
  );
  const taskStops = toolUses.filter((use) => use.name === "TaskStop");
  const foregroundRuntime = bashCommands.filter((use) => isRuntimeStartCommand(use.command) && use.input.run_in_background !== true);
  const backgroundRuntime = bashCommands.filter((use) => isRuntimeStartCommand(use.command) && use.input.run_in_background === true);

  const issues = [];
  addIssue(issues, "error", "hook_schema_validation", hookErrors.filter((item) => item.type === "hook_schema_validation"));
  addIssue(issues, "error", "forbidden_plan_mode_tool", forbiddenPlanModeTools);
  addIssue(issues, "error", "bare_loom_command", bareLoom);
  addIssue(issues, "error", "loom_cli_missing_claude_profile", missingClaudeProfile);
  addIssue(issues, "warning", "loom_cli_missing_compact_output", missingCompact);
  addIssue(issues, "warning", "stop_guard_intervened", stopGuardBlocks);
  addIssue(issues, "warning", "malformed_read_tool_input", malformedReads);
  addIssue(issues, "warning", "malformed_write_tool_input", malformedWrites);
  addIssue(issues, "warning", "foreground_runtime_start", foregroundRuntime);
  addIssue(issues, "warning", "large_tool_output", largeOutputs.slice(0, 20));
  addIssue(issues, "info", "taskstop_runtime_cleanup", taskStops);
  addIssue(issues, "info", "claude_internal_implementation_aid", internalImplementationAids);
  addIssue(issues, "info", "background_runtime_start", backgroundRuntime);
  addIssue(issues, "info", "direct_loom_artifact_read", fullArtifactReads.slice(0, 20));

  return {
    summary: {
      totalLines: events.length,
      toolUses: toolUses.length,
      bashCommands: bashCommands.length,
      loomCommands: loomCommands.length,
      hookErrors: hookErrors.length,
      stopGuardBlocks: stopGuardBlocks.length,
      taskStops: taskStops.length,
      malformedReads: malformedReads.length,
      malformedWrites: malformedWrites.length,
      largeOutputs: largeOutputs.length,
      foregroundRuntimeStarts: foregroundRuntime.length,
      backgroundRuntimeStarts: backgroundRuntime.length,
    },
    issues,
  };
}

function addIssue(issues, severity, code, evidence) {
  if (!evidence.length) return;
  issues.push({ severity, code, count: evidence.length, evidence: evidence.slice(0, 10).map(compactEvidence) });
}

function compactEvidence(item) {
  const out = { ...item };
  if (out.input) out.input = truncateValue(out.input);
  if (out.command) out.command = snippet(out.command, 300);
  if (out.message) out.message = snippet(out.message, 300);
  if (out.excerpt) out.excerpt = snippet(out.excerpt, 300);
  return out;
}

function truncateValue(value) {
  const text = JSON.stringify(value);
  if (text.length <= 500) return value;
  return `${text.slice(0, 500)}...`;
}

function eventText(value) {
  const parts = [];
  const content = value.message?.content;
  if (typeof content === "string") parts.push(content);
  if (Array.isArray(content)) {
    for (const item of content) {
      if (typeof item === "string") parts.push(item);
      else if (item?.text) parts.push(item.text);
      else if (typeof item?.content === "string") parts.push(item.content);
    }
  }
  if (value.attachment?.stderr) parts.push(value.attachment.stderr);
  if (value.attachment?.stdout) parts.push(value.attachment.stdout);
  if (value.attachment?.content) parts.push(String(value.attachment.content));
  if (value.toolUseResult) parts.push(JSON.stringify(value.toolUseResult));
  return parts.join("\n");
}

function isRuntimeStartCommand(command) {
  const trimmed = command.trim();
  return /^(?:[A-Za-z_][A-Za-z0-9_]*=\S+\s+)*(npm|pnpm|yarn|bun)\s+run\s+(start|dev|preview|serve)\b/.test(trimmed) ||
    /^(?:[A-Za-z_][A-Za-z0-9_]*=\S+\s+)*(vite|next\s+dev|node\s+\S*server|python(?:3)?\s+-m\s+(uvicorn|http\.server)|uvicorn|gunicorn)\b/.test(trimmed);
}

function firstNonEmpty(...values) {
  return values.find((value) => String(value || "").trim()) || "";
}

function snippet(text, max = 300) {
  const value = String(text || "").replace(/\s+/g, " ").trim();
  return value.length > max ? `${value.slice(0, max)}...` : value;
}

function printReport(projectRoot, sessionFile, findings) {
  console.log(`Claude Loom session audit`);
  console.log(`Project: ${projectRoot}`);
  console.log(`Session: ${sessionFile}`);
  console.log(`Summary: ${JSON.stringify(findings.summary)}`);
  if (!findings.issues.length) {
    console.log("No adapter-boundary findings detected.");
    return;
  }
  for (const issue of findings.issues) {
    console.log(`\n[${issue.severity}] ${issue.code} (${issue.count})`);
    for (const item of issue.evidence) {
      const at = [item.line ? `line ${item.line}` : null, item.timestamp].filter(Boolean).join(" ");
      const detail = item.message || item.command || item.filePath || item.name || JSON.stringify(item.input || {});
      console.log(`- ${at}: ${detail}`);
    }
  }
}
