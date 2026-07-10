const PENDING_TTL_MS = 30 * 60 * 1000;
const IDLE_PROMPT_COOLDOWN_MS = 15 * 1000;

export const LoomPlugin = async ({ client, directory }) => {
  const pendingBySession = new Map();
  const promptLocks = new Map();

  function remember(sessionID, pending) {
    pendingBySession.set(sessionID, {
      ...pending,
      createdAt: Date.now(),
      lastPromptAt: 0,
    });
  }

  function clear(sessionID) {
    pendingBySession.delete(sessionID);
    promptLocks.delete(sessionID);
  }

  return {
    "command.execute.before": async (input) => {
      if (input.command === "loom" || input.command === "loom-deploy") {
        remember(input.sessionID, {
          projectRoot: directory,
          state: "prompt",
          signature: `prompt:${input.command}:${Date.now()}`,
        });
      }
    },

    "tool.execute.after": async (input, output) => {
      const result = extractActionResult(output.output);
      if (!result || !isLoomActionResult(result)) {
        return;
      }
      const state = String(result.state || "unknown");
      if (state === "auto_runnable") {
        const pending = {
          projectRoot: projectRootFromResult(result, directory),
          state,
          nextKind: result.next?.kind || null,
          requestRef: result.next?.requestRef || result.requestRef || null,
          submitTool: result.next?.submitTool || null,
          writeTargets: result.next?.writeTargets || [],
          signature: signatureForResult(result),
        };
        remember(input.sessionID, pending);
        output.title = "Loom MCP continuation required";
        output.metadata = {
          ...(output.metadata ?? {}),
          loomAutoRunnable: true,
          loomNextKind: pending.nextKind,
          loomRequestRef: pending.requestRef,
        };
        output.output = `${continuationBanner(pending)}\n\n${output.output}`;
        return;
      }
      if (state === "active_operation") {
        remember(input.sessionID, {
          projectRoot: projectRootFromResult(result, directory),
          state,
          nextKind: "observe",
          signature: signatureForResult(result),
        });
        return;
      }
      if (["user_gate", "repairable_error"].includes(state)) {
        remember(input.sessionID, {
          projectRoot: projectRootFromResult(result, directory),
          state,
          nextKind: state,
          signature: signatureForResult(result),
        });
        return;
      }
      clear(input.sessionID);
    },

    event: async ({ event }) => {
      if (event.type !== "session.idle") {
        return;
      }
      const sessionID = event.properties.sessionID;
      const pending = pendingBySession.get(sessionID);
      if (!pending) {
        return;
      }
      if (Date.now() - pending.createdAt > PENDING_TTL_MS) {
        clear(sessionID);
        return;
      }
      if (!["auto_runnable", "active_operation"].includes(pending.state)) {
        return;
      }
      const lock = promptLocks.get(sessionID);
      if (lock && Date.now() - lock < IDLE_PROMPT_COOLDOWN_MS) {
        return;
      }
      promptLocks.set(sessionID, Date.now());
      await client.session.promptAsync({
        path: { id: sessionID },
        query: { directory: pending.projectRoot || directory },
        body: {
          agent: "build",
          system: "Continue the active Loom MCP workflow. Do not summarize, mark a local plan complete, send a final answer, or ask whether to continue while the latest result is auto-runnable or active.",
          parts: [
            {
              type: "text",
              metadata: { loomAutoContinue: true, signature: pending.signature },
              text: idlePrompt(pending),
            },
          ],
        },
      });
    },
  };
};

function extractActionResult(text) {
  if (typeof text !== "string") {
    return null;
  }
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

function isLoomActionResult(result) {
  return typeof result === "object" && typeof result.state === "string";
}

function projectRootFromResult(result, fallback) {
  return result.projectRoot || result.next?.projectRoot || fallback;
}

function signatureForResult(result) {
  return [
    result.state,
    result.next?.kind || "",
    result.next?.requestRef || "",
    result.next?.submitTool || "",
  ].join(":");
}

function continuationBanner(pending) {
  return [
    "Loom MCP returned an auto-runnable next action.",
    `Next kind: ${pending.nextKind || "unknown"}.`,
    pending.requestRef ? `Request: ${pending.requestRef}.` : null,
    "Continue with the returned MCP action before reporting completion.",
  ].filter(Boolean).join("\n");
}

function idlePrompt(pending) {
  if (pending.state === "active_operation") {
    return "The latest Loom MCP result is active_operation. Call only the observation tools named by that result, then continue following the returned state.";
  }
  if (pending.nextKind === "run_loom_tool") {
    return [
      "The latest Loom MCP result is auto_runnable.",
      "Execute next.kind=run_loom_tool now.",
      pending.requestRef ? `Use requestRef=${pending.requestRef}.` : null,
      "Inspect the request, read only the returned readGroups, call the returned Loom MCP tool, then retry the returned retryTool before reporting progress.",
    ].filter(Boolean).join("\n");
  }
  return [
    "The latest Loom MCP result is auto_runnable.",
    `Execute next.kind=${pending.nextKind || "unknown"} now.`,
    pending.requestRef ? `Use requestRef=${pending.requestRef}.` : null,
    "Read declared request groups through Loom MCP read tools, write only returned targets, and submit through the returned MCP submit tool before reporting completion.",
  ].filter(Boolean).join(" ");
}
