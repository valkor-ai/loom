# Agent Run Benchmark

This benchmark prepares paired workspaces for real coding-agent comparisons:

- `direct`: a minimal direct coding-agent prompt with the same request/context.
- `loom`: a workspace with a real `loom plan --compact` request already created,
  plus prompt guardrails for `.loom` artifact-guided execution.

The runner does not invoke Codex, Claude, or another agent automatically. That keeps
the benchmark independent from local account state, model access, and UI surface.
Instead it creates prompts, metadata, and result templates so runs can be executed
and recorded consistently.

## Prepare Runs

From the Loom package root:

```bash
node benchmarks/agent-run/run.js prepare --skip-build
```

Useful options:

```bash
node benchmarks/agent-run/run.js prepare --skip-build --repeat 3
node benchmarks/agent-run/run.js prepare --case backend-readiness-continuation
node benchmarks/agent-run/run.js prepare --case billing-entitlements-continuation
node benchmarks/agent-run/run.js prepare --case compliance-evidence-continuation
node benchmarks/agent-run/run.js prepare --case customer-onboarding-continuation
node benchmarks/agent-run/run.js prepare --case feature-flags-continuation
node benchmarks/agent-run/run.js prepare --case fulfillment-operations-continuation
node benchmarks/agent-run/run.js prepare --case incident-review-continuation
node benchmarks/agent-run/run.js prepare --case analytics-funnel-continuation
node benchmarks/agent-run/run.js prepare --case release-readiness-continuation
node benchmarks/agent-run/run.js prepare --case support-sla-continuation
node benchmarks/agent-run/run.js prepare --case workspace-permissions-continuation
node benchmarks/agent-run/run.js prepare --case support-sla-continuation --repeat 3
node benchmarks/agent-run/run.js prepare --out-dir /tmp/loom-agent-run-benchmark
node benchmarks/agent-run/run.js prepare --agent-profile codex
```

The command prints the generated run directory. Each variant contains:

- `PROMPT.md`: paste or use this prompt in the target agent surface.
- `RESULT_TEMPLATE.json`: fill this after the agent run, or use `record`.
- `metadata.json`: case, variant, prompt, workspace, and Loom request metadata.
- `workspace/`: isolated project workspace for that run.

Seeded cases also include a verification command in the prompt and result template.
Run that command from the variant workspace before recording `passed`.

## Record Results

```bash
node benchmarks/agent-run/run.js record \
  --variant-dir /tmp/loom-agent-run-benchmark/run-.../cases/backend-readiness-continuation/loom \
  --status passed \
  --turns 4 \
  --repair-loops 1 \
  --tests passed \
  --verification-command "npm test && node benchmark-verify.js" \
  --verification-status passed \
  --tokens-used 74064 \
  --changed-file workspace/src/readiness.js \
  --success-criteria-met 5 \
  --success-criteria-total 5 \
  --notes "Completed with one self-repair."
```

Completion should be recorded for every real run. Token savings are only useful
when delivery completion is equal or close:

- `successCriteriaMet` / `successCriteriaTotal` records task completion.
- `verification.status` records whether the required verifier passed.
- `notes` can capture short run observations when needed.

## Summarize

```bash
node benchmarks/agent-run/run.js summarize --run-dir /tmp/loom-agent-run-benchmark/run-...
node benchmarks/agent-run/run.js summarize --run-dir /tmp/loom-agent-run-benchmark/run-... --markdown-out /tmp/loom-agent-run-benchmark.md
node benchmarks/agent-run/run.js summarize --run-dir /tmp/loom-agent-run-benchmark/run-a --run-dir /tmp/loom-agent-run-benchmark/run-b
```

Use `prepare --repeat 3` or summarize multiple `--run-dir` values when comparing
agent surfaces. The aggregate table reports paired runs, Loom token wins, median
token savings, mean token savings, and completion deltas.

When both `direct` and `loom` variants have numeric token usage, `summarize`
also reports a paired token comparison:

- `Loom Saved` is `directTokens - loomTokens`.
- `Completion Delta` is `loomCompletion - directCompletion`.
