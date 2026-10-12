---
name: project-architecture
description: Draw or explain the target project's software architecture and data flows with source-backed diagrams. Use when the user asks for a project architecture diagram, a subsystem walkthrough, or a visual explanation of a code change.
---

# Project Architecture

Diagram the project Loom is delivering. Use its application, service, module,
scene, storage, and integration boundaries. Loom's planning stages, task results,
and review records belong to a delivery-evidence view, not the target system's
technical architecture.

## Establish the architecture

Start with the existing project context: architecture contracts, repository
context, documentation, manifests, and code already inspected for the task.
For an implemented system, use those records to locate the relevant code and
confirm key relationships. Fill gaps with focused file or symbol lookups;
use an existing code index or language server when available. Do not build a
repository scanner or read every file merely to draw the diagram.

Choose boundaries that help explain the requested system. A project overview
should include its surrounding application and runtime boundaries; a subsystem
view can focus on the affected components. Tests can appear in a supporting
view without becoming production services.

Attach source files and relevant line ranges to implemented nodes and edges.
A call, import dependency, HTTP request, data read/write, and shared participation
in a workflow are different relationships. Label them accordingly. Co-occurrence
in a task or contract does not prove a call. Leave unresolved links out and
mention the missing evidence in the explanation.

For a new project, draw the declared design and label the diagram as proposed.
For an existing project, draw the implementation supported by the inspected
source. If comparing design and implementation, keep them in separate views.
Describe exactly what the selected source supports rather than claiming a whole
system from a local repair contract.

## Render with PR Lens

Read [references/pr-lens.md](references/pr-lens.md) when producing the diagram.
The current agent authors the graph document; PR Lens validates and renders it.
The renderer checks graph structure, not the truth of the architecture claims.

Use architectural lanes such as client, API, domain, storage, or external systems.
Add a data-flow view only when the interaction order is supported by code or an
explicit design. A sequence animation illustrates that order; it is not a runtime
trace. Keep generated IDs in the document and readable technical names on nodes.

Export local SVGs and inspect their layout before handing them over. Include the
JSON so a later request can update the diagram without recreating the analysis.
Record the inspected revision and any relevant uncommitted changes alongside
the output. When reusing a graph, follow the source freshness guidance in
[references/pr-lens.md](references/pr-lens.md); a valid graph can still describe
old code.
Run locally by default; publishing a hosted canvas or posting to a PR requires
the user's request for that destination.

When invoked during a Loom delivery, follow the current MCP action and preserve
its contract. Diagram outputs are supporting artifacts; they do not replace
verification evidence or rewrite canonical `.loom` records. This skill is not
an automatic step in every delivery.
