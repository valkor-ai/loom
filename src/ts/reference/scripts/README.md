# Legacy TypeScript Adapter Scripts

These scripts are preserved only because the TypeScript reference tests compare historical CLI adapter behavior.

They are not product install scripts. Current Loom installation goes through release packages and `loom-setup`, which installs the Rust MCP server, bundled Python algorithm runtime, and MCP plugin templates.

Do not call these scripts from product code, plugin templates, release packaging, or MCP tools.
