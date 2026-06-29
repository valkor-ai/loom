<div align="center">
  <img src="./assets/headbar.png" alt="Loom" width="100%">
  <p><strong>Loop engineering for agentic software delivery.</strong></p>
  <p>An open delivery harness that turns Claude Code, Codex, OpenCode and other coding agents into repeatable software delivery systems.</p>
  <p>
    <a href="./README.zh-CN.md">Simplified Chinese</a>
    ·
    <a href="https://zonodqioyxil6r3k.public.blob.vercel-storage.com/Loomline-v0.pdf">Technical Report</a>
    ·
    <a href="./docs/use-cases.md">Use Cases</a>
    ·
    <a href="#quick-start">Quick Start</a>
    ·
    <a href="#how-to-use">How to Use</a>
    ·
    <a href="#token-saving-context">Token Saving</a>
    ·
    <a href="#related-work">Related Work</a>
    ·
    <a href="#faq">FAQ</a>
  </p>
  <p>
    <a href="./LICENSE"><img alt="License: Apache-2.0" src="https://img.shields.io/badge/License-Apache--2.0-blue.svg"></a>
    <a href="https://discord.gg/Yr7UjwbYPC"><img alt="Discord" src="https://img.shields.io/badge/Discord-Join-5865F2?logo=discord&logoColor=white"></a>
    <img alt="Rust" src="https://img.shields.io/badge/Rust-MCP%20runtime-b7410e?logo=rust&logoColor=white">
    <img alt="Python" src="https://img.shields.io/badge/Python-algorithms-3776AB?logo=python&logoColor=white">
    <img alt="Status" src="https://img.shields.io/badge/status-open-brightgreen">
  </p>
</div>

## What Is Loom?

Loom is an open-source delivery harness for existing coding agents. It does not replace the model or editor you already use; it turns each delivery goal into a structured loop of planning, building, verification, repair, preview, and handoff.

Loom uses dynamic workflows to choose the right delivery path for each goal, then makes that path durable: project context, task contracts, backend state, test results, preview evidence, repair notes, and handoff reports are persisted so the next session or agent can continue without starting over.

Instead of a one-shot prompt chain, Loom treats delivery as a loop: route the next step, execute, verify, record evidence, repair when needed, and continue from saved state.

Coding agents can write code. Loom helps them keep the delivery promise from idea to release, with fewer wasted tokens.

Use Loom when a request is larger than a one-shot edit: a feature needs clarification, architecture, task splitting, implementation evidence, review, repair, preview, deployment, or a clean handoff.

## Why a Harness?

Website and app generation is becoming table stakes. The harder problem is reliable delivery: keeping the agent aligned after compaction, preserving requirements across many turns, verifying its own work without bias, repairing failures, and resuming from the right step after an interruption.

Long-running agent work tends to break down in predictable ways:

Failure mode | Loom response
--- | ---
Partial completion | Bounded tasks, explicit result files, continue routing, and final-response guards keep agents from declaring done after partial progress.
Goal drift | Confirmed scope, architecture contracts, task plans, and compact context packs preserve the original objective across sessions.
Self-check bias | Review, verification, repair requests, and evidence records separate implementation from validation.
Token waste | Project summaries, task graphs, backend/runtime state, test results, and deployment evidence reduce repeated whole-repo reads.
Handoff gaps | Delivery reports, preview checks, logs, and repair history make the final state inspectable by humans and other agents.

The hard part is the harness around the model: durable state, scoped work, routing, verification, recovery, and human-readable evidence. Loom uses dynamic workflows as the operating pattern, then lifts them to the project level so delivery can survive interruptions, compaction, agent switches, and future handoffs.

That is where Loom is different from prompt files, one-off workflows, and single-agent scripts: it stores delivery state in `.loom/`, exposes an MCP tool protocol to coding agents, and makes verification, repair, preview, and handoff first-class protocol steps.

## From Demo to Delivery

Vibe Coding and AI Coding are making software creation accessible to more builders than ever. More people can now turn an idea into a demo, prototype a product, or build a tool for themselves with the help of coding agents.

But there is still a large gap between a demo that works once and a production-grade application that can be trusted, shipped, repaired, and evolved.

That gap is not only about model capability. Even as models improve, builders still need to clarify requirements, preserve project context, make architectural decisions, prepare backend/runtime state, run checks, inspect failures, repair issues, verify again, preview the result, and collect delivery evidence.

Loom exists to close that gap.

It is an open-source delivery layer for existing coding agents. It helps agents move from one-shot coding to repeatable software delivery: clarify the request, plan the work, split tasks, preserve context, execute checks, repair failures, and report evidence.

The goal is simple: help builders move from vibe-coded demos and personal tools to reliable, production-ready applications with less manual effort and fewer wasted tokens.

Capability | What it changes
--- | ---
Dynamic workflows | Turn each delivery goal into an adaptive loop for clarification, planning, execution, verification, repair, and handoff.
Delivery harness | Route work through requirement clarification, planning, building, checking, previewing, reviewing, repairing, and reporting.
Requirement intelligence | Turns clarification from a chat step into a delivery-quality gate: confirmed scope, business rules, lifecycle coverage, and UI operation paths become structured context that planning, execution, and review must preserve.
Knowledge-guided clarification | Lets teams register local domain docs as named knowledge sources, build searchable local indexes, and let requirement clarification pull only matching chunks into the right step without making the knowledge base a hidden requirement source.
Token-saving context | Persist project summaries, task graphs, backend/runtime state, tests, and deployment results so agents do not reread the whole repository every turn.
Task contracts | Turn broad goals into bounded tasks with source refs, acceptance intent, result files, and continuation rules.
Executable tools | Give agents MCP tools for context collection, task routing, result recording, deployment checks, and delivery evidence.
Backend readiness | Track databases, auth, storage, functions, environment variables, services, and runtime requirements as part of the delivery state.
UIX guidance | Preserve visual direction, interaction flows, responsive states, accessibility expectations, and product-specific interface details as delivery requirements.
Verification loop | Turn smoke tests, Playwright-style checks, logs, error summaries, repair requests, and re-verification into a repeatable loop.
Multi-agent protocol | Bring the same delivery process to Claude Code, Codex, OpenCode and other agents.

## Token-Saving Context

High-level context path:

```text
Your coding agent / app
(Codex, Claude Code, OpenCode, future agents...)
        |
        | delivery goal . repo context . logs . tests . preview evidence
        v
+----------------------------------------------------------------------------+
| Loom  (project-local delivery state; full artifacts stay in .loom/)         |
|----------------------------------------------------------------------------|
| Dynamic workflow router -> Request manifest -> Agent read plan              |
|                              |                                             |
|                              |- requestReadPlan     grouped required reads  |
|                              |- MCP field resources targeted retrieval      |
|                              |- write targets       authorized artifact I/O |
|                              `- action result       next tool + compact view |
|                                                                            |
| Task contracts . evidence windows . fullLogRef . review/repair/resume state |
+----------------------------------------------------------------------------+
        |
        | compact instruction + selected field groups + retrieval path
        v
Agent turn / LLM context
```

In the latest 11-case agent-run benchmark, Codex + Loom used 15.8% fewer tokens than Codex alone while preserving 100% completion. See the [latest benchmark results](./benchmarks/agent-run/results/latest.md) and the [run guide](./benchmarks/agent-run/README.md).

## Prerequisites

- One supported coding agent installed locally: Codex, Claude Code, or OpenCode
- Docker for `loom deploy`

## Quick Start

Install Loom for the coding agent you use. The installer detects your OS and CPU, downloads the matching release package, verifies the package `.sha256` asset, installs the Rust MCP server, bundles the Python algorithm runtime, writes the agent MCP registration, refreshes the local plugin, and runs `loom-setup doctor`.

Codex:

```bash
curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh | bash -s -- --agent codex
```

Claude Code:

```bash
curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh | bash -s -- --agent claude-code
```

OpenCode:

```bash
curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh | bash -s -- --agent opencode
```

All supported agents on the same machine:

```bash
curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh | bash -s -- --agent all
```

Windows PowerShell:

```powershell
Invoke-WebRequest https://github.com/valkor-ai/loom/releases/latest/download/install.ps1 -OutFile install.ps1
.\install.ps1 -Agent codex
```

Run the same install command again to upgrade. The installer removes Loom-owned legacy CLI plugin artifacts before installing the MCP runtime. If it finds files it cannot prove are Loom-owned, it stops and tells you what to remove manually instead of overwriting user files.

For local validation from a repository checkout, use the same installer in local build mode:

```bash
./install.sh --agent codex --local-build
```

This builds the Rust MCP server and setup binary, writes the same release package layout, then installs through `loom-setup` and runs doctor. Use this path after local bug fixes so the installer, package layout, MCP registration, and plugin refresh are verified together.

After installing or updating an agent plugin, open a new agent session in the target project so the refreshed MCP registration and plugin files are loaded.

To verify the install without starting a delivery, use the Loom command inside your coding agent:

```text
@loom status     # Codex
/loom status     # Claude Code and OpenCode
```

`status` is read-only. In a project that has not used Loom yet, `STATE_NOT_INITIALIZED` is a valid smoke-check result: it means the plugin command is available and no delivery has been started.

You normally do not initialize `.loom/` by hand. Starting a delivery from the agent, such as `@loom build ...` or `/loom build ...`, initializes the project-local delivery state when needed.

## How to Use

Loom is meant to be used through the local plugin inside your coding agent. Use `@loom` in Codex and `/loom` in Claude Code or OpenCode. The Rust MCP server is started by the agent MCP registration; users do not start it by hand.

### Use Knowledge Sources

Knowledge sources are optional, but they are useful when your delivery work depends on product rules, domain notes, design standards, operating procedures, or other local reference material.

Loom treats knowledge sources as clarification aids, not as requirements by themselves. During requirement clarification, Loom searches enabled and successfully built knowledge indexes, reads only matching chunks for the current clarification step, and turns useful findings into user-visible questions or confirmation points.

Run knowledge commands from the coding agent session for the project you are working on. The examples below show Codex with `@loom`; in Claude Code and OpenCode, use the same subcommands with `/loom`.

Add a new knowledge source:

```text
@loom knowledge add --name product-rules ~/Documents/product-rules
@loom knowledge build product-rules
```

`--name` is required and must be unique. A source can include one file, many files, one directory, many directories, or a mix of files and directories. Currently supported formats are `.md`, `.txt`, `.json`, `.yaml`, `.yml`, `.pdf`, and `.docx`.

Update an existing knowledge source's registered paths:

```text
@loom knowledge update product-rules --add-path ~/Documents/new-rules.md
@loom knowledge update product-rules --remove-path ~/Documents/old-rules.md
@loom knowledge update product-rules --replace-paths ~/Documents/current-rules
@loom knowledge build product-rules
```

If the files inside an already registered path changed, run `build` again. You do not need `update` unless the path set changes.

Resume an unfinished semantic knowledge build:

```text
@loom knowledge resume product-rules
```

Use `resume` when a knowledge build stopped before publishing, for example after reopening a coding-agent session or when a multi-pack semantic build did not finish. It does not rebuild the source; it finds the next unfinished semantic pack and lets the agent continue until the index is published.

Review and manage existing knowledge sources:

```text
@loom knowledge list
@loom knowledge status product-rules
@loom knowledge pending product-rules
@loom knowledge discard product-rules
```

Disable a source without deleting it:

```text
@loom knowledge disable product-rules
@loom knowledge enable product-rules
```

Remove a source registration and its local Loom index:

```text
@loom knowledge remove product-rules
```

`remove` does not delete your original documents. It only removes Loom's registration, pending queue, and built index for that knowledge source.

### Run Delivery

Start from your coding agent with its Loom command surface:

Codex:

```text
@loom build a visitor registration system
@loom continue
@loom review
@loom deploy
```

Claude Code and OpenCode:

```text
/loom build a visitor registration system
/loom continue
/loom review
/loom deploy
```

In all agents, the command starts the same Loom MCP delivery protocol. The plugin routes the request to Loom tools and follows the structured next action returned by the MCP server.
For new delivery requests, the explicit `plan` subcommand is equivalent to a bare request: `@loom plan build ...` matches `@loom build ...`, and `/loom plan build ...` matches `/loom build ...`.

Use `continue` whenever you want Loom to resume or advance the current delivery safely. This is the right first action after reopening an agent session, after an interruption, after a tool action succeeds but the agent does not keep going, or when you are not sure which step is next.

```text
@loom continue     # Codex
/loom continue     # Claude Code and OpenCode
```

Agent plugins set the Loom routing environment for you. Use the agent command surface for normal work; Loom's product runtime is the MCP server installed by `loom-setup`.

## How It Works

Loom creates project-local delivery state under `.loom/` and uses it as the source of truth for the agent's next action. The core loop is short:

1. Capture and confirm the delivery scope.
2. Build a compact context pack.
3. Generate planning, architecture, and task contracts.
4. Execute one bounded task at a time.
5. Record evidence and run verification.
6. Review, repair, and re-check.
7. Report the final delivery state.

## Learn More

Need | Command or file
--- | ---
Check Loom plugin availability | `@loom status` in Codex, or `/loom status` in Claude Code and OpenCode
Install or upgrade Codex plugin | `curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh \| bash -s -- --agent codex`
Install or upgrade Claude Code plugin | `curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh \| bash -s -- --agent claude-code`
Install or upgrade OpenCode plugin | `curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh \| bash -s -- --agent opencode`
Install or upgrade all supported plugins | `curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh \| bash -s -- --agent all`
Run a local deployment preview | `@loom deploy` in Codex, or `/loom deploy` in Claude Code and OpenCode

## FAQ

<details>
<summary>How is Loom different from <code>CLAUDE.md</code>, <code>AGENTS.md</code>, or <code>.cursorrules</code>?</summary>

Those files are useful entry points, but they tend to become large prompts. Loom adds stateful delivery routing, task artifacts, review results, repair requests, deployment evidence, and MCP tools around them.

</details>

<details>
<summary>What happens if a delivery is interrupted?</summary>

Loom stores project-local delivery state under `.loom/`, including context, task plans, result records, review notes, repair requests, and deployment evidence. Reopen the agent session and run `@loom continue` in Codex or `/loom continue` in Claude Code and OpenCode; Loom will route the next step from the saved delivery state.

</details>

<details>
<summary>Does Loom deploy to production?</summary>

Not yet. Production deployment will be added later. Current deployment support focuses on local Docker Compose previews, validation, logs, and repair guidance.

</details>

## Uninstalling Loom

If you need to remove Loom from one local agent, use `loom-setup`:

```bash
~/.loom/bin/loom-setup uninstall --agent codex
~/.loom/bin/loom-setup uninstall --agent claude-code
~/.loom/bin/loom-setup uninstall --agent opencode
```

To remove all local Loom agent plugins from this machine:

```bash
~/.loom/bin/loom-setup uninstall --all
```

To remove Loom user-level runtime data, including installed runtimes and user-level knowledge indexes:

```bash
~/.loom/bin/loom-setup purge
```

`uninstall` keeps project-local `.loom/` delivery state. `purge` is intentionally broader and should be used only when you want to remove Loom's user-level runtime and indexes from this machine.

After uninstalling a plugin, open a new agent session so that agent reloads its local command/plugin state.

## Related Work

Loom is informed by adjacent work in coding-agent skills, agentic engineering workflows, and software engineering evaluation:

- [Matt Pocock's Skills](https://github.com/mattpocock/skills) - Practical agent skills for requirement clarification, domain language, debugging, TDD, and handoff discipline.
- [SWE-bench](https://github.com/SWE-bench/SWE-bench) - Real-world software engineering tasks used to evaluate coding agents.

## Supported By

<img src="https://zonodqioyxil6r3k.public.blob.vercel-storage.com/logo/Zhejiang_University_Logo.svg" alt="Zhejiang University" width="220"> <img src="https://zonodqioyxil6r3k.public.blob.vercel-storage.com/logo/University_College_London_logo.svg" alt="University College London" width="220">

## Star History

<a href="https://www.star-history.com/#valkor-ai/loom&Date">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=valkor-ai/loom&type=Date&theme=dark" />
    <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=valkor-ai/loom&type=Date" />
    <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=valkor-ai/loom&type=Date" />
  </picture>
</a>

## License

Loom is open source under the [Apache License 2.0](./LICENSE).
