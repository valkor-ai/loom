# PR Lens rendering

PR Lens provides the graph schema, validator, and SVG renderer. Use an existing
compatible installation when available. The commands below use the tested
`@coldtea/pr-lens-cli@0.13.0` release and require Node.js 20.11 or newer. Keep
instruction, validation, and rendering commands on the same version.

## Load the contract

```bash
npx --yes @coldtea/pr-lens-cli@0.13.0 skill
npx --yes @coldtea/pr-lens-cli@0.13.0 skill references
```

The installed CLI supplies the field definitions and example graph. Read those
before writing a document instead of guessing schema fields. Adapt the upstream
workflow to the local output requested here; the upstream suggestion to push a
canvas does not authorize uploading the project's graph.

## Author the document

- Use the target project's identity and source revision. Derive provenance from
  the actual repository; omit optional values that are unknown. Never substitute
  Loom's repository, an invented PR, or made-up base/head commits.
- For a system snapshot use `unchanged` deltas. For a change view derive deltas
  from the chosen diff, keeping surrounding components for context.
  The graph schema requires repository identity and base/head revisions. For a
  snapshot set both revisions to the inspected commit. If repository identity
  or revision cannot be established, use Mermaid rather than inventing them.
- Put concrete component names on nodes. Use `files` to bind them to the source;
  summaries explain responsibilities. Put an edge's source evidence in its
  `files` and label its actual relationship.
- For a proposed design, name the view accordingly and cite the design artifact.
  Do not present a contract-only module as a confirmed implementation.
- Leave `flows`, payloads, walkthroughs, and statistics out unless they improve
  the requested explanation and are supported by the available information.

## Keep source evidence current

Save a short source note beside the graph: target repository, inspected commit,
view scope, and whether cited files include staged, unstaged, or untracked
changes. Record content hashes of cited files when they contain uncommitted
changes, so the commit alone is not mistaken for the inspected source. Keep
these notes outside the strict PR Lens document schema. A diagram is a saved
snapshot, not a live code index.

When a user asks to reuse or update a diagram, compare the recorded commit with
the current revision and check relevant working-tree changes. Use existing Git
diffs and the graph's file references to locate affected nodes and edges. Include
changed task-owned entry points, configuration, or neighbouring modules when
they may introduce a new boundary; cited-file changes alone cannot detect every
new dependency. A different commit does not by itself make every relationship
obsolete.

Recheck affected source relationships and refresh their file ranges before
rendering an updated view. Keep stable IDs for components whose identity has
not changed. A rename requires checking its callers; a removed component requires
updating its incident edges and any referencing views or flows. Preserve
unaffected portions when the available evidence supports them. If the previous
source revision or working-tree snapshot is unavailable, report that freshness
is unverified and inspect the requested scope before presenting it as current.

For a refreshed snapshot, set base and head to the newly inspected commit and
keep deltas `unchanged`. For a change diagram, preserve the user-selected base
and head comparison. Revalidate the JSON and inspect the rendered output after
changes. Update on request or when the diagram is needed for the active task;
do not add a recurring scan or an obligatory delivery step.

## Validate and render

Create a task-specific scratch directory outside the target checkout, for
example `/tmp/loom-project-architecture/<project>/`. Write `graph.json` there.
Run from that directory so PR Lens's scratch-directory handling does not edit
the target project's `.gitignore`.

```bash
npx --yes @coldtea/pr-lens-cli@0.13.0 validate graph.json
npx --yes @coldtea/pr-lens-cli@0.13.0 render graph.json \
  --out rendered --theme light --no-config
```

Fix validation failures in the graph document. If a needed relationship has no
source evidence, resolve it or explain its omission; do not invent a link to
make validation pass. Read the output filenames from `rendered/manifest.json`
or the CLI output. Inspect the SVG for clipping, unreadable labels, crossings,
and accidental mixing of design and implementation before sharing it.

These commands need no additional model API key. Package downloads still need
network access. If Node or the package is unavailable, report that limitation
and provide a Mermaid view of the same verified relationships instead of
writing another renderer.

Upstream: https://github.com/coldteadotai/pr-lens (MIT). This skill uses the
published CLI; it does not vendor the upstream renderer or skill text.
