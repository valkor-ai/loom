# Delivery graph

For source-backed diagrams of the target project, use the shared
[project-architecture skill](../plugins/shared/loom/skills/project-architecture/SKILL.md).
It reuses the task's code context and uses PR Lens to validate and render a local
graph document. The record-based exports below show saved delivery evidence and
declared designs.

Call the `deliveryGraph` MCP tool with `projectRoot` to read the active delivery (or the most recently completed delivery). The tool returns `nodes`, `edges`, and a `mermaid` flowchart. Paste the Mermaid text into a Markdown `mermaid` fence to view the diagram. It reads `.loom/` records on demand and never modifies delivery state.

The graph uses explicit references from the architecture contract, task plan, task results, verification results, and review. A file changed by a task is connected to that task result, not directly to an architecture module. This is a delivery-evidence graph, not a code call graph. Missing stages produce a partial graph.

## Local SVG export

From a Loom source checkout, export the selected project's delivery as a standalone SVG:

```bash
cargo run --manifest-path src/rust/Cargo.toml -p mcp-server \
  --bin loom-delivery-graph -- /absolute/path/to/project /tmp/delivery.svg
```

Open the SVG in a browser. Nodes are grouped into acceptance, tasks, architecture, execution, evidence, and review. Solid lines show execution and evidence relationships; dashed lines show planned coverage. Hover over a node for its full ID or over a connection for its relationship. The export reflects recorded verification and review statuses; it does not run checks again.

This exporter runs locally on demand. The binary is built from source and is not included in the current release installer.

## Technical architecture

Use the architecture view to show applications, modules, interfaces and interactions, data ownership, entity relationships, and flow references from the architecture contract:

```bash
cargo run --manifest-path src/rust/Cargo.toml -p mcp-server \
  --bin loom-delivery-graph -- /absolute/path/to/project /tmp/architecture.svg \
  --view architecture
```

To inspect a saved phase in the selected delivery, add `--phase phase-2`. Without this option the exporter selects the active phase. Module responsibilities and declared communication protocols appear on the cards, with full text available on hover.

Connections come from explicit provider/consumer references, ownership entries, read models, entity relationships, and flow references. A module's presence in an application or a flow does not establish a module-to-module call. The source-of-truth card describes the declared persistence boundary; it does not imply a database or a deployment topology. This view reads the saved architecture contract, so it represents that phase's design rather than a live trace of the running system.
