import { promises as fs } from "node:fs";
import path from "node:path";
import { stateCorrupted, unsupportedSchemaVersion } from "../errors";
import { SUPPORTED_SCHEMA_VERSIONS } from "../../version";

export async function pathExists(filePath: string): Promise<boolean> {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

export async function ensureDir(dirPath: string): Promise<boolean> {
  try {
    const stat = await fs.stat(dirPath);
    if (!stat.isDirectory()) {
      throw stateCorrupted("loom state path exists but is not a directory.", {
        path: dirPath,
      });
    }
    return false;
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code !== "ENOENT") {
      throw error;
    }
  }

  await fs.mkdir(dirPath, { recursive: true });
  return true;
}

export async function writeJsonIfMissing(filePath: string, value: unknown): Promise<boolean> {
  if (await pathExists(filePath)) {
    return false;
  }
  await writeJsonAtomic(filePath, value);
  return true;
}

export async function writeTextIfMissing(filePath: string, value: string): Promise<boolean> {
  if (await pathExists(filePath)) {
    return false;
  }
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  const tmp = `${filePath}.tmp-${process.pid}-${Date.now()}`;
  await fs.writeFile(tmp, value, "utf8");
  await fs.rename(tmp, filePath);
  return true;
}

export async function writeJsonAtomic(filePath: string, value: unknown): Promise<void> {
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  const tmp = `${filePath}.tmp-${process.pid}-${Date.now()}`;
  await fs.writeFile(tmp, `${JSON.stringify(value, null, 2)}\n`, "utf8");
  await fs.rename(tmp, filePath);
}

export async function writeTextAtomic(filePath: string, value: string): Promise<void> {
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  const tmp = `${filePath}.tmp-${process.pid}-${Date.now()}`;
  await fs.writeFile(tmp, value, "utf8");
  await fs.rename(tmp, filePath);
}

export async function readJsonFile(filePath: string): Promise<unknown> {
  let raw: string;
  try {
    raw = await fs.readFile(filePath, "utf8");
  } catch {
    throw stateCorrupted("loom JSON file cannot be read.", { file: filePath });
  }

  try {
    return JSON.parse(raw);
  } catch {
    throw stateCorrupted("loom JSON file contains invalid JSON.", { file: filePath });
  }
}

export async function readJsonWithSchemaVersion(filePath: string): Promise<unknown> {
  let raw: string;
  try {
    raw = await fs.readFile(filePath, "utf8");
  } catch (error) {
    throw error;
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw stateCorrupted("loom state file contains invalid JSON.", { file: filePath });
  }

  if (typeof parsed !== "object" || parsed === null || !("schemaVersion" in parsed)) {
    throw stateCorrupted("loom state file is missing schemaVersion.", { file: filePath });
  }

  const schemaVersion = (parsed as { schemaVersion: unknown }).schemaVersion;
  if (typeof schemaVersion !== "number") {
    throw stateCorrupted("loom state file has invalid schemaVersion.", {
      file: filePath,
      found: schemaVersion,
    });
  }

  if (!SUPPORTED_SCHEMA_VERSIONS.includes(schemaVersion as 1)) {
    throw unsupportedSchemaVersion({
      file: filePath,
      found: schemaVersion,
      supported: SUPPORTED_SCHEMA_VERSIONS,
    });
  }

  return parsed;
}
