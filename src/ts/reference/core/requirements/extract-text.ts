import { createHash } from "node:crypto";
import { promises as fs } from "node:fs";
import path from "node:path";
import mammoth from "mammoth";
import { PDFParse } from "pdf-parse";
import yauzl from "yauzl";

export type ExtractedRequirementText = {
  text: string;
  mimeType: string;
  status: "completed" | "unsupported" | "failed";
  reason?: string;
  digest: string;
};

export async function extractRequirementText(filePath: string): Promise<ExtractedRequirementText> {
  const absolutePath = path.resolve(filePath);
  const buffer = await fs.readFile(absolutePath);
  const ext = path.extname(absolutePath).toLowerCase();
  const mimeType = mimeTypeForExtension(ext);
  const digest = `sha256:${hashBuffer(buffer)}`;

  try {
    if (isPlainTextExtension(ext)) {
      return { text: buffer.toString("utf8"), mimeType, status: "completed", digest };
    }
    if (ext === ".pdf") {
      return { text: await extractPdf(buffer), mimeType, status: "completed", digest };
    }
    if (ext === ".docx") {
      const result = await mammoth.extractRawText({ path: absolutePath });
      return { text: result.value, mimeType, status: "completed", digest };
    }
    if (ext === ".xlsx") {
      return { text: await extractXlsx(absolutePath), mimeType, status: "completed", digest };
    }
    if (ext === ".doc" || ext === ".xls") {
      return {
        text: "",
        mimeType,
        status: "unsupported",
        reason: `${ext} binary extraction is not supported without conversion; provide .docx/.xlsx or text alongside it.`,
        digest,
      };
    }
    return {
      text: tryDecodeText(buffer),
      mimeType,
      status: "completed",
      digest,
    };
  } catch (error) {
    return {
      text: "",
      mimeType,
      status: "failed",
      reason: error instanceof Error ? error.message : String(error),
      digest,
    };
  }
}

export function mimeTypeForPath(filePath: string): string {
  return mimeTypeForExtension(path.extname(filePath).toLowerCase());
}

export function digestText(text: string): string {
  return `sha256:${createHash("sha256").update(text).digest("hex")}`;
}

function hashBuffer(buffer: Buffer): string {
  return createHash("sha256").update(buffer).digest("hex");
}

function mimeTypeForExtension(ext: string): string {
  switch (ext) {
    case ".pdf":
      return "application/pdf";
    case ".docx":
      return "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
    case ".doc":
      return "application/msword";
    case ".xlsx":
      return "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
    case ".xls":
      return "application/vnd.ms-excel";
    case ".md":
    case ".markdown":
      return "text/markdown";
    case ".json":
      return "application/json";
    case ".yaml":
    case ".yml":
      return "application/yaml";
    case ".txt":
      return "text/plain";
    default:
      return "application/octet-stream";
  }
}

function isPlainTextExtension(ext: string): boolean {
  return [
    ".txt",
    ".md",
    ".markdown",
    ".json",
    ".yaml",
    ".yml",
    ".csv",
    ".tsv",
    ".html",
    ".xml",
    ".ts",
    ".tsx",
    ".js",
    ".jsx",
    ".py",
    ".java",
    ".go",
    ".rs",
  ].includes(ext);
}

async function extractPdf(buffer: Buffer): Promise<string> {
  const parser = new PDFParse({ data: buffer });
  try {
    const result = await parser.getText();
    return result.text;
  } finally {
    await parser.destroy();
  }
}

async function extractXlsx(filePath: string): Promise<string> {
  const entries = await readZipEntries(filePath);
  const sharedStrings = parseSharedStrings(entries.get("xl/sharedStrings.xml") ?? "");
  const sheetNames = [...entries.keys()]
    .filter((entry) => /^xl\/worksheets\/sheet\d+\.xml$/.test(entry))
    .sort(naturalCompare);
  const parts: string[] = [];
  for (const sheetName of sheetNames) {
    const sheetText = parseSheetText(entries.get(sheetName) ?? "", sharedStrings);
    if (sheetText.trim()) {
      parts.push(`# ${sheetName}\n${sheetText}`);
    }
  }
  return parts.join("\n\n");
}

async function readZipEntries(filePath: string): Promise<Map<string, string>> {
  const zip = await openZipFile(filePath);
  const result = new Map<string, string>();
  try {
    await new Promise<void>((resolve, reject) => {
      zip.readEntry();
      zip.on("entry", (entry: yauzl.Entry) => {
        if (/\/$/.test(entry.fileName)) {
          zip.readEntry();
          return;
        }
        zip.openReadStream(entry, (error, stream) => {
          if (error || !stream) {
            reject(error ?? new Error(`Unable to read zip entry ${entry.fileName}`));
            return;
          }
          const chunks: Buffer[] = [];
          stream.on("data", (chunk) => chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk)));
          stream.on("error", reject);
          stream.on("end", () => {
            result.set(entry.fileName, Buffer.concat(chunks).toString("utf8"));
            zip.readEntry();
          });
        });
      });
      zip.on("end", resolve);
      zip.on("error", reject);
    });
  } finally {
    zip.close();
  }
  return result;
}

function openZipFile(filePath: string): Promise<yauzl.ZipFile> {
  return new Promise((resolve, reject) => {
    yauzl.open(filePath, { lazyEntries: true }, (error, zip) => {
      if (error || !zip) {
        reject(error ?? new Error(`Unable to open zip file ${filePath}`));
        return;
      }
      resolve(zip);
    });
  });
}

function parseSharedStrings(xml: string): string[] {
  return [...xml.matchAll(/<si\b[\s\S]*?<\/si>/g)].map((match) => xmlText(match[0]));
}

function parseSheetText(xml: string, sharedStrings: string[]): string {
  const rows = [...xml.matchAll(/<row\b[\s\S]*?<\/row>/g)];
  return rows
    .map((rowMatch) => {
      const cells = [...rowMatch[0].matchAll(/<c\b([^>]*)>([\s\S]*?)<\/c>/g)];
      return cells
        .map((cellMatch) => {
          const attrs = cellMatch[1] ?? "";
          const body = cellMatch[2] ?? "";
          const rawValue = body.match(/<v>([\s\S]*?)<\/v>/)?.[1] ?? body.match(/<t[^>]*>([\s\S]*?)<\/t>/)?.[1] ?? "";
          if (!rawValue) {
            return "";
          }
          if (/\bt="s"/.test(attrs)) {
            return sharedStrings[Number(rawValue)] ?? "";
          }
          if (/\bt="inlineStr"/.test(attrs)) {
            return xmlText(body);
          }
          return decodeXml(rawValue);
        })
        .filter(Boolean)
        .join("\t");
    })
    .filter(Boolean)
    .join("\n");
}

function xmlText(xml: string): string {
  return [...xml.matchAll(/<t[^>]*>([\s\S]*?)<\/t>/g)]
    .map((match) => decodeXml(match[1] ?? ""))
    .join("");
}

function decodeXml(value: string): string {
  return value
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, "\"")
    .replace(/&apos;/g, "'")
    .replace(/&amp;/g, "&");
}

function naturalCompare(a: string, b: string): number {
  return a.localeCompare(b, undefined, { numeric: true });
}

function tryDecodeText(buffer: Buffer): string {
  const text = buffer.toString("utf8");
  return text.includes("\u0000") ? "" : text;
}
