import path from "node:path";
import { promises as fs } from "node:fs";
import { parse } from "yaml";
import { pathExists } from "../state/fs";
import type { DeploymentComposeInfo, DeploymentComposePort, DeploymentComposeService } from "./types";

export type ExistingDeploymentFiles = {
  dockerfilePath: string | null;
  composePath: string | null;
};

const composeFileNames = [
  "compose.yaml",
  "compose.yml",
  "docker-compose.yaml",
  "docker-compose.yml",
];

const dockerfileNames = [
  "Dockerfile",
  "dockerfile",
];

export async function findExistingDeploymentFiles(projectRoot: string): Promise<ExistingDeploymentFiles> {
  return {
    dockerfilePath: await findFirstExisting(projectRoot, dockerfileNames),
    composePath: await findFirstExisting(projectRoot, composeFileNames),
  };
}

export async function detectComposePublishedPort(
  composePath: string,
): Promise<{ hostPort: number; containerPort: number } | null> {
  const analysis = await analyzeExistingCompose(composePath);
  const port = selectedComposePort(analysis);
  if (!port?.hostPort) {
    return null;
  }

  return {
    hostPort: port.hostPort,
    containerPort: port.containerPort,
  };
}

export async function analyzeExistingCompose(composePath: string): Promise<DeploymentComposeInfo> {
  const raw = await fs.readFile(composePath, "utf8");
  try {
    const document = parse(raw) as unknown;
    const root = isRecord(document) ? document : {};
    const services = isRecord(root.services) ? root.services : {};
    const serviceEntries = Object.entries(services);

    if (serviceEntries.length === 0) {
      return emptyComposeInfo("Compose file has no services block.");
    }

    const analyzedServices = serviceEntries.map(([name, service]) => analyzeComposeService(name, service));
    const selectedService = selectComposeService(analyzedServices);

    return {
      selectedService: selectedService?.name ?? null,
      serviceReason: selectedService?.reason ?? "No application service could be selected from the Compose file.",
      services: analyzedServices,
      warnings: selectedService?.dependencyLike
        ? [`Selected service ${selectedService.name} looks dependency-like; inspect the existing Compose file before relying on this deployment.`]
        : [],
    };
  } catch (error) {
    return emptyComposeInfo(error instanceof Error ? `Could not parse Compose file: ${error.message}` : "Could not parse Compose file.");
  }
}

export function selectedComposePort(info: DeploymentComposeInfo): DeploymentComposePort | null {
  const service = info.services.find((candidate) => candidate.name === info.selectedService);
  return service?.ports.find((port) => port.hostPort !== null) ?? service?.ports[0] ?? null;
}

async function findFirstExisting(projectRoot: string, names: string[]): Promise<string | null> {
  for (const name of names) {
    const candidate = path.join(projectRoot, name);
    if (await pathExists(candidate)) {
      return candidate;
    }
  }
  return null;
}

function analyzeComposeService(name: string, service: unknown): DeploymentComposeService {
  const value = isRecord(service) ? service : {};
  const image = typeof value.image === "string" ? value.image : null;
  const ports = parseComposePorts(value.ports);
  const expose = parseExpose(value.expose);
  const dependsOn = parseDependsOn(value.depends_on);
  const profiles = parseStringList(value.profiles);
  const build = value.build !== undefined;
  const dependencyLike = isDependencyLikeService(name, image, ports);
  const signals: string[] = [];
  let score = 0;

  if (isAppServiceName(name)) {
    score += 70;
    signals.push(`service name ${name} looks like an application service`);
  }
  if (build) {
    score += 45;
    signals.push("has build configuration");
  }
  if (ports.some((port) => port.hostPort !== null)) {
    score += 35;
    signals.push("publishes a host port");
  } else if (ports.length > 0) {
    score += 20;
    signals.push("declares service ports");
  }
  if (expose.length > 0) {
    score += 10;
    signals.push("exposes internal ports");
  }
  if (dependsOn.length > 0) {
    score += 5;
    signals.push("depends on other services");
  }
  if (dependencyLike) {
    score -= 90;
    signals.push("looks like an infrastructure dependency");
  }
  if (profiles.some((profile) => /test|ci|debug/i.test(profile))) {
    score -= 20;
    signals.push("is behind a test/debug profile");
  }
  if (signals.length === 0) {
    signals.push("no strong service signals");
  }

  return {
    name,
    score,
    image,
    build,
    ports,
    expose,
    dependsOn,
    profiles,
    dependencyLike,
    reason: signals.join("; "),
  };
}

function selectComposeService(services: DeploymentComposeService[]): DeploymentComposeService | null {
  const sorted = [...services].sort((left, right) => right.score - left.score || left.name.localeCompare(right.name));
  return sorted.find((service) => !service.dependencyLike) ?? sorted[0] ?? null;
}

function parseComposePorts(value: unknown): DeploymentComposePort[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value
    .map((port) => {
      if (typeof port === "string" || typeof port === "number") {
        return parsePortString(String(port));
      }
      if (isRecord(port)) {
        const target = numericPort(port.target);
        if (!target) {
          return null;
        }
        const published = numericPort(port.published);
        return {
          hostPort: published,
          containerPort: target,
          protocol: typeof port.protocol === "string" ? port.protocol : null,
          raw: JSON.stringify(port),
        };
      }
      return null;
    })
    .filter((port): port is DeploymentComposePort => Boolean(port));
}

function parsePortString(raw: string): DeploymentComposePort | null {
  const trimmed = raw.trim();
  const protocolMatch = trimmed.match(/\/([a-z]+)$/i);
  const protocol = protocolMatch?.[1] ?? null;
  const withoutProtocol = trimmed.replace(/\/[a-z]+$/i, "");
  const parts = withoutProtocol.split(":");
  const numericParts = parts
    .map((part) => numericPort(part))
    .filter((port): port is number => port !== null);

  if (numericParts.length === 0) {
    return null;
  }

  const containerPort = numericParts[numericParts.length - 1];
  const hostPort = numericParts.length >= 2 ? numericParts[numericParts.length - 2] : null;
  return {
    hostPort,
    containerPort,
    protocol,
    raw: trimmed,
  };
}

function parseExpose(value: unknown): number[] {
  return parseStringList(value)
    .map((entry) => numericPort(entry))
    .filter((port): port is number => port !== null);
}

function parseDependsOn(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value.filter((entry): entry is string => typeof entry === "string");
  }
  if (isRecord(value)) {
    return Object.keys(value);
  }
  return [];
}

function parseStringList(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value.map(String);
  }
  if (typeof value === "string" || typeof value === "number") {
    return [String(value)];
  }
  return [];
}

function numericPort(value: unknown): number | null {
  if (typeof value === "number" && Number.isInteger(value) && value > 0) {
    return value;
  }
  if (typeof value !== "string") {
    return null;
  }
  const match = value.match(/\d{2,5}/);
  if (!match) {
    return null;
  }
  const port = Number(match[0]);
  return Number.isInteger(port) && port > 0 && port <= 65_535 ? port : null;
}

function isAppServiceName(name: string): boolean {
  return /^(app|web|api|server|backend|frontend|client|www|site|gateway|service)$/i.test(name);
}

function isDependencyLikeService(
  name: string,
  image: string | null,
  ports: DeploymentComposePort[],
): boolean {
  const text = `${name} ${image ?? ""}`.toLowerCase();
  if (/(postgres|postgresql|mysql|mariadb|redis|mongo|rabbitmq|elasticsearch|opensearch|minio|memcached|kafka|zookeeper|localstack|mailhog|mailpit|meilisearch|typesense|clickhouse|influxdb|neo4j|mssql|sqlserver)/.test(text)) {
    return true;
  }
  return ports.some((port) => [5432, 3306, 6379, 27017, 5672, 9200, 9000, 11211, 9092, 2181].includes(port.containerPort));
}

function emptyComposeInfo(reason: string): DeploymentComposeInfo {
  return {
    selectedService: null,
    serviceReason: reason,
    services: [],
    warnings: [reason],
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
