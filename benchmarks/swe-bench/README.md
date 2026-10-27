# SWE-bench

This runner prepares comparable local workspaces for direct Codex and Codex with Loom. It supports SWE-bench Lite and SWE-bench Verified. It does not run Codex for you: each agent session remains interactive. It records the task identity, prompt, patch, delivery evidence, and local evaluation outcome in a consistent local format.

All downloaded dataset data, cached repositories, worktrees, patches, and logs stay local under this directory and are ignored by Git.

Before collecting a result intended for publication, follow the [benchmark protocol](./PROTOCOL.md). The protocol fixes task selection, agent settings, evaluation, and reporting so a result measures a delivery outcome rather than a hand-picked run.

## Prepare A Run

```bash
node benchmarks/swe-bench/run.mjs prepare \
  --dataset verified \
  --run september-smoke \
  --model gpt-5.6-sol \
  --provider openai \
  --reasoning high \
  --max-turns 40 \
  --max-elapsed-ms 3600000 \
  --instance pallets__flask-5014 \
  --instance django__django-10554
```

The first `prepare` downloads and caches the selected dataset metadata. Omit `--dataset` for Lite, or use `--dataset verified` for SWE-bench Verified. It creates a clean checkout at each task's recorded base commit:

```text
benchmarks/swe-bench/runs/september-smoke/<instance>/direct/
benchmarks/swe-bench/runs/september-smoke/<instance>/loom/
```

Open each directory as a separate Codex task and use its `PROMPT.md`. The prompts intentionally exclude SWE-bench gold and test patches.
Each workspace is a one-commit shallow checkout with no Git remote. The runner's cached repository may contain later upstream history, but that history is not available to an agent during a run.

Before starting an agent, create the task's local environment using that repository's pinned dependencies. For Python tasks, a typical setup is `python3 -m venv .venv && .venv/bin/python -m pip install -e .`; install any task-specific test dependencies as needed. Then launch both variants through the generated wrapper:

```bash
./run-agent.sh codex exec --json -C . "$(<PROMPT.md)"
```

`run-agent.sh` prevents local shell startup files from replacing the task environment. When `.venv` exists, it makes that environment the default for commands that an agent runs. This avoids accidentally testing an unrelated editable install from the host machine.

## Record An Agent Run

After an agent stops, save its patch and the metrics available from its session:

```bash
node benchmarks/swe-bench/run.mjs record \
  --run september-smoke \
  --instance pytest-dev__pytest-11143 \
  --variant loom \
  --status completed \
  --tokens 42000 \
  --turns 12 \
  --repairs 1 \
  --elapsed-ms 840000
```

Use the same command with `--variant direct` for the direct Codex workspace. `tokens`, `turns`, and `repairs` are optional because availability differs by agent and provider.

## Run Local Evaluation

`evaluate` creates a separate clean checkout, applies the recorded agent patch and then the SWE-bench test patch. Provide the task-specific command required by that repository's pinned environment. Use `--setup` when the clean checkout needs dependencies before its tests can run:

```bash
node benchmarks/swe-bench/run.mjs evaluate \
  --run september-smoke \
  --instance pytest-dev__pytest-11143 \
  --variant loom \
  --setup 'uv venv .venv --python 3.12 && uv pip install --python .venv/bin/python -e .' \
  --command '.venv/bin/python -m pytest testing/test_assertrewrite.py'
```

This command is local evidence only. Run it in an isolated environment: never install an editable task checkout into a shared Python environment. The official SWE-bench Docker harness remains the authority for any published score. This runner keeps the dataset test patch outside agent workspaces and preserves patches for later official evaluation.

Some agent patches add regression coverage in the same file or hunk touched by the hidden SWE-bench test patch. When that prevents the two patches from applying together, rerun local evaluation with one or more `--exclude-agent-path` values:

```bash
node benchmarks/swe-bench/run.mjs evaluate \
  --run september-smoke \
  --instance pytest-dev__pytest-11143 \
  --variant loom \
  --exclude-agent-path testing/test_assertrewrite.py \
  --command '.venv/bin/python -m pytest testing/test_assertrewrite.py'
```

The runner retains the full agent patch in `artifacts/<variant>.patch` and generates `artifacts/<variant>.filtered.patch` for this local check. It applies the hidden test patch first and records the result as `passed (filtered)` in the summary. Filtered checks verify the agent's non-excluded implementation changes only; they are not official SWE-bench results and must be reported as local evidence.

## Record Official Evaluation

After running the official SWE-bench harness, record its outcome and copy its JSON report into the local run evidence:

```bash
node benchmarks/swe-bench/run.mjs official \
  --run september-smoke \
  --instance pytest-dev__pytest-11143 \
  --variant loom \
  --status resolved \
  --report /absolute/path/to/report.json
```

Use `resolved` only when the official harness resolves the instance. All other official outcomes are `unresolved`.

## Summarize

```bash
node benchmarks/swe-bench/run.mjs summarize --run september-smoke
```

This writes `SUMMARY.md` beside the local run data. Because coding-agent executions are stochastic, compare repeated runs using completion and local-evaluation pass rates; treat token and elapsed-time values as distributions, not fixed values.
