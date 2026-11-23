# Agent Permissions

- Map which actor may read data, select tools, write files, approve actions, and publish results. Check approval against the exact tool, arguments, destination, and candidate version.
- Confirm that retrieved documents, tool results, logs, and generated text do not become authority for a subsequent action. Inspect how trusted runtime metadata is kept separate from task content.
- Check that retries, resumed sessions, and delegated workers keep the original scope. Review rejection of stale requests and unexpected output destinations.
- Treat a prompt-only restriction as guidance, not proof that a filesystem, tool, or service boundary is enforced.
