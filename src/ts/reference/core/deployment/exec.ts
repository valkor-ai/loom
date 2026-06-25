import { spawn } from "node:child_process";

export type ExecResult = {
  exitCode: number;
  stdout: string;
  stderr: string;
};

export type ExecOptions = {
  cwd: string;
  timeoutMs?: number;
};

export async function execFile(
  command: string,
  args: string[],
  options: ExecOptions,
): Promise<ExecResult> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd,
      stdio: ["ignore", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";
    let settled = false;
    const timeout = options.timeoutMs
      ? setTimeout(() => {
          if (settled) {
            return;
          }
          settled = true;
          child.kill("SIGTERM");
          resolve({
            exitCode: 124,
            stdout,
            stderr: `${stderr}${stderr ? "\n" : ""}Command timed out after ${options.timeoutMs}ms.`,
          });
        }, options.timeoutMs)
      : null;

    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });

    child.on("error", (error) => {
      if (settled) {
        return;
      }
      settled = true;
      if (timeout) {
        clearTimeout(timeout);
      }
      reject(error);
    });

    child.on("close", (exitCode) => {
      if (settled) {
        return;
      }
      settled = true;
      if (timeout) {
        clearTimeout(timeout);
      }
      resolve({
        exitCode: exitCode ?? 1,
        stdout,
        stderr,
      });
    });
  });
}
