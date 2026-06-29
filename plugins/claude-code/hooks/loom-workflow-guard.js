#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

main();

function main() {
  const input = readJsonFromStdin();
  const eventName = String(input.hook_event_name || "");
  const toolName = String(input.tool_name || "");
  const sessionId = String(input.session_id || "");
  const cwd = path.resolve(input.cwd || process.env.CLAUDE_PROJECT_DIR || process.cwd());

  if (eventName === "UserPromptSubmit" && isLoomPrompt(input.user_prompt)) {
    writeSession(sessionId, cwd, { active: true, state: "prompt", stopAllowed: false });
    return;
  }

  if (eventName === "PreToolUse") {
    if (isLoomMcpTool(toolName)) {
      writeSession(sessionId, cwd, { active: true, state: "tool_call", stopAllowed: false });
      return;
    }
    if ((toolName === "EnterPlanMode" || toolName === "ExitPlanMode") && isActiveAutoRunnable(sessionId, cwd)) {
      deny(`Loom MCP workflow is active. Continue with the latest Loom MCP action result instead of entering ${toolName}.`);
      return;
    }
  }

  if (eventName === "PostToolUse" && isLoomMcpTool(toolName)) {
    const result = extractActionResult(input.tool_response) || extractActionResult(input.tool_result) || extractActionResult(input);
    if (!result) {
      writeSession(sessionId, cwd, { active: true, state: "unknown", stopAllowed: false });
      return;
    }
    const state = String(result.state || "unknown");
    writeSession(sessionId, cwd, {
      active: !["done", "blocked", "failed"].includes(state),
      state,
      nextKind: result.next?.kind || null,
      stopAllowed: ["user_gate", "done", "blocked", "failed", "repairable_error"].includes(state),
      updatedAt: new Date().toISOString(),
    });
    return;
  }

  if (eventName === "Stop") {
    const session = readSession(sessionId, cwd);
    if (!session || session.stopAllowed === true) {
      return;
    }
    if (session.state === "auto_runnable") {
      blockStop(`Loom MCP returned auto_runnable${session.nextKind ? ` (${session.nextKind})` : ""}. Execute the returned next action before stopping.`);
      return;
    }
    if (session.state === "active_operation") {
      blockStop("Loom MCP has an active operation. Use only the observation tools named by the latest result before stopping.");
    }
  }
}

function readJsonFromStdin() {
  try {
    const raw = fs.readFileSync(0, "utf8").trim();
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function isLoomPrompt(value) {
  return /(^|\s)\/loom(?:-deploy)?(\s|$)/i.test(String(value || ""));
}

function isLoomMcpTool(toolName) {
  const text = String(toolName || "").toLowerCase();
  return text.includes("loom.") || text.includes("mcp__loom");
}

function extractActionResult(value) {
  if (!value) {
    return null;
  }
  if (typeof value === "object" && typeof value.state === "string") {
    return value;
  }
  const text = typeof value === "string" ? value : JSON.stringify(value);
  const first = text.indexOf("{");
  const last = text.lastIndexOf("}");
  if (first < 0 || last <= first) {
    return null;
  }
  try {
    const parsed = JSON.parse(text.slice(first, last + 1));
    if (typeof parsed.state === "string") {
      return parsed;
    }
    if (typeof parsed.structuredContent?.state === "string") {
      return parsed.structuredContent;
    }
    if (typeof parsed.content?.[0]?.text === "string") {
      return extractActionResult(parsed.content[0].text);
    }
  } catch {
    return null;
  }
  return null;
}

function sessionPath(sessionId) {
  if (!sessionId) {
    return null;
  }
  const home = process.env.LOOM_HOME || (process.env.HOME ? path.join(process.env.HOME, ".loom") : "");
  if (!home) {
    return null;
  }
  const safe = sessionId.replace(/[^A-Za-z0-9_.-]/g, "_").slice(0, 160) || "unknown";
  return path.join(home, "agent-sessions", "claude-code", `${safe}.json`);
}

function writeSession(sessionId, cwd, patch) {
  const target = sessionPath(sessionId);
  if (!target) {
    return;
  }
  const existing = readSession(sessionId, cwd) || {};
  const next = {
    schemaVersion: 1,
    sessionId,
    cwd,
    ...existing,
    ...patch,
    updatedAt: new Date().toISOString(),
  };
  try {
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, `${JSON.stringify(next, null, 2)}\n`);
  } catch {
    // Hook failures must not break ordinary Claude usage.
  }
}

function readSession(sessionId, cwd) {
  const target = sessionPath(sessionId);
  if (!target) {
    return null;
  }
  try {
    const value = JSON.parse(fs.readFileSync(target, "utf8"));
    if (value.sessionId !== sessionId || path.resolve(value.cwd || "") !== cwd) {
      return null;
    }
    return value;
  } catch {
    return null;
  }
}

function isActiveAutoRunnable(sessionId, cwd) {
  const session = readSession(sessionId, cwd);
  return session && session.stopAllowed !== true && session.active === true;
}

function deny(message) {
  writeJson({
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: message,
    },
    systemMessage: message,
  });
}

function blockStop(reason) {
  writeJson({
    decision: "block",
    reason,
    systemMessage: "Loom MCP continuation guard blocked an incomplete stop.",
  });
}

function writeJson(value) {
  process.stdout.write(`${JSON.stringify(value)}\n`);
}
