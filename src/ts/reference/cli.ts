#!/usr/bin/env node

import { runCli } from "./commands";
import { toFailureEnvelope } from "./commands/envelope";
import { printEnvelope } from "./commands/output";
import { diagnosticProjectRoot } from "./commands/safe-cwd";
import { exitCodeForErrorCode } from "./core/errors";

runCli(process.argv.slice(2)).catch((error) => {
  const projectRoot = diagnosticProjectRoot();
  const failure = toFailureEnvelope("unknown", projectRoot, error, { argv: process.argv.slice(2) });
  printEnvelope(failure);
  process.exitCode = exitCodeForErrorCode(failure.error.code);
});
