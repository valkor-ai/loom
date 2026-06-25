import path from "node:path";

export function safeCwd(): string | null {
  try {
    return process.cwd();
  } catch {
    return null;
  }
}

export function diagnosticProjectRoot(cwd = safeCwd()): string {
  if (cwd) {
    return cwd;
  }
  const pwd = process.env.PWD;
  return pwd && path.isAbsolute(pwd) ? path.normalize(pwd) : "/";
}
