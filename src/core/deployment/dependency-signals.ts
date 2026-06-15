import type { DependencyService, DependencyServiceKind, DetectedStack } from "./types";

export type DependencyServiceSignalMatch = {
  kind: DependencyServiceKind;
  reason: string;
  database: boolean;
};

export const DATABASE_SERVICE_KINDS = new Set<DependencyServiceKind>(["postgres", "mysql", "mongodb"]);
export const SQL_SERVICE_KINDS = new Set<DependencyServiceKind>(["postgres", "mysql"]);

const dependencyManifestPattern = /package\.json$|pom\.xml$|build\.gradle|requirements\.txt$|pyproject\.toml$|go\.mod$|\.csproj$|composer\.json$|Gemfile$/;

const looseDependencySignals: Record<DependencyServiceKind, string[]> = {
  postgres: [
    "postgres",
    "postgresql",
    "org.postgresql",
    "jdbc:postgresql",
    "npgsql",
    "pdo_pgsql",
    "pgsql",
    "psycopg",
    "asyncpg",
    "prisma",
    "drizzle-orm",
    "gorm.io/driver/postgres",
  ],
  redis: ["redis", "ioredis", "bullmq", "bull-board", "predis", "phpredis", "sidekiq", "spring-data-redis", "lettuce", "jedis", "stackexchange.redis"],
  mysql: ["mysql", "mysql2", "mariadb", "jdbc:mysql", "jdbc:mariadb", "mysqlconnector", "pdo_mysql", "mysqli", "pymysql", "mysqlclient", "gorm.io/driver/mysql"],
  mongodb: ["mongodb", "mongoose", "pymongo", "motor", "mongodb.driver", "spring-boot-starter-data-mongodb", "spring-data-mongodb", "go.mongodb.org/mongo-driver"],
  rabbitmq: ["rabbitmq", "amqplib", "amqp", "pika", "rabbitmq.client", "spring-rabbit"],
  elasticsearch: ["elasticsearch", "@elastic/elasticsearch", "elastic.clients.elasticsearch", "org.elasticsearch", "opensearch"],
  minio: ["minio", "s3_endpoint", "aws_s3_endpoint", "s3-compatible"],
};

const runtimeContractPatterns: Array<{ kind: DependencyServiceKind; pattern: RegExp }> = [
  { kind: "postgres", pattern: /postgres|postgresql|pgsql|jdbc:postgresql/ },
  { kind: "mysql", pattern: /mysql|mariadb|jdbc:mysql|jdbc:mariadb/ },
  { kind: "redis", pattern: /redis|redis_url|spring_redis|spring_data_redis/ },
  { kind: "mongodb", pattern: /mongodb|mongo|mongodb_url|spring_data_mongodb/ },
  { kind: "rabbitmq", pattern: /rabbitmq|amqp|rabbitmq_url|spring_rabbit/ },
  { kind: "elasticsearch", pattern: /elasticsearch|opensearch/ },
  { kind: "minio", pattern: /minio|s3_endpoint|s3-compatible/ },
];

const evidencePatterns: Array<{ kind: DependencyServiceKind; reason: string; database: boolean; pattern: RegExp; packageNames?: string[] }> = [
  {
    kind: "postgres",
    reason: "PostgreSQL driver or connection signal found.",
    database: true,
    pattern: /jdbc:postgresql|postgresql:\/\/|postgres:\/\/|adapter:\s*postgresql|org\.postgresql|gorm\.io\/driver\/postgres|psycopg|asyncpg|npgsql|pdo_pgsql|pgsql/,
    packageNames: ["pg"],
  },
  {
    kind: "mysql",
    reason: "MySQL/MariaDB driver or connection signal found.",
    database: true,
    pattern: /jdbc:mysql|jdbc:mariadb|mysql:\/\/|mariadb:\/\/|adapter:\s*mysql2?|mysql-connector|mysql2|pymysql|mysqlclient|pdo_mysql|mysqli|gorm\.io\/driver\/mysql/,
  },
  {
    kind: "redis",
    reason: "Redis driver, queue, or connection signal found.",
    database: false,
    pattern: /redis:\/\/|redis_url|spring\.data\.redis|spring_redis|ioredis|bullmq|lettuce|jedis|stackexchange\.redis|predis|phpredis|sidekiq|gem\s+["']redis["']/,
    packageNames: ["redis"],
  },
  {
    kind: "mongodb",
    reason: "MongoDB driver or connection signal found.",
    database: true,
    pattern: /mongodb:\/\/|mongodb|mongoose|pymongo|motor|mongo-driver|spring-boot-starter-data-mongodb/,
  },
  {
    kind: "rabbitmq",
    reason: "RabbitMQ/AMQP driver or connection signal found.",
    database: false,
    pattern: /rabbitmq|amqp:\/\/|rabbitmq_url|spring_rabbit|amqplib|pika/,
  },
  {
    kind: "elasticsearch",
    reason: "Elasticsearch/OpenSearch driver or endpoint signal found.",
    database: false,
    pattern: /elasticsearch|opensearch|elastic\.clients|@elastic\/elasticsearch/,
  },
  {
    kind: "minio",
    reason: "MinIO/S3-compatible endpoint signal found.",
    database: false,
    pattern: /minio|s3_endpoint|aws_s3_endpoint|s3-compatible/,
  },
];

export function dependencyServiceKindsFromLooseSignals(signals: string): DependencyServiceKind[] {
  const normalized = signals.toLowerCase();
  const kinds: DependencyServiceKind[] = [];
  for (const kind of Object.keys(looseDependencySignals) as DependencyServiceKind[]) {
    if (looseDependencySignals[kind].some((needle) => normalized.includes(needle))) {
      kinds.push(kind);
      continue;
    }
    if (kind === "postgres" && hasTokenSignal(normalized, "pg")) {
      kinds.push(kind);
    }
  }
  return dedupeKinds(kinds);
}

export function detectedDependencyReason(kind: DependencyServiceKind): string {
  switch (kind) {
    case "postgres":
      return "Detected postgres/pg/prisma/drizzle signal.";
    case "redis":
      return "Detected redis/ioredis/queue signal.";
    case "mysql":
      return "Detected mysql/mysql2/mariadb signal.";
    case "mongodb":
      return "Detected mongodb/mongoose/pymongo signal.";
    case "rabbitmq":
      return "Detected rabbitmq/amqp signal.";
    case "elasticsearch":
      return "Detected elasticsearch/opensearch signal.";
    case "minio":
      return "Detected minio/s3-compatible storage signal.";
  }
}

export function dependencyServiceKindsFromRuntimeSignals(signals: string): DependencyServiceKind[] {
  const normalized = signals.toLowerCase();
  return dedupeKinds(runtimeContractPatterns
    .filter((entry) => entry.pattern.test(normalized))
    .map((entry) => entry.kind));
}

export function dependencyServiceEvidenceMatches(input: {
  path: string;
  text: string;
  lower: string;
}): DependencyServiceSignalMatch[] {
  const matches: DependencyServiceSignalMatch[] = [];
  for (const rule of evidencePatterns) {
    const packageMatch = rule.packageNames ? hasPackageDependency(input.path, input.text, rule.packageNames) : false;
    if (rule.pattern.test(input.lower) || packageMatch) {
      matches.push({
        kind: rule.kind,
        reason: rule.reason,
        database: rule.database,
      });
    }
  }
  return matches;
}

export function hasPackageDependency(relativePath: string, text: string, names: string[]): boolean {
  if (!relativePath.endsWith("package.json")) {
    return false;
  }
  try {
    const pkg = JSON.parse(text) as {
      dependencies?: Record<string, unknown>;
      devDependencies?: Record<string, unknown>;
      optionalDependencies?: Record<string, unknown>;
    };
    const deps = {
      ...(pkg.dependencies ?? {}),
      ...(pkg.devDependencies ?? {}),
      ...(pkg.optionalDependencies ?? {}),
    };
    return names.some((name) => Object.prototype.hasOwnProperty.call(deps, name));
  } catch {
    return false;
  }
}

export function prismaProvider(text: string): string | null {
  const match = text.match(/provider\s*=\s*"([^"]+)"/i);
  return match?.[1]?.toLowerCase() ?? null;
}

export function hasSqliteSignal(relativePath: string, text: string, lower: string): boolean {
  return (relativePath.endsWith("schema.prisma") && prismaProvider(text) === "sqlite") ||
    /jdbc:sqlite|sqlite:\/\/|sqlite3|better-sqlite3|microsoft\.data\.sqlite/.test(lower);
}

export function databaseRuntimeSignalLabel(lower: string): string | null {
  if (/spring[_\.]datasource/.test(lower)) return "SPRING_DATASOURCE";
  if (/database_url/.test(lower)) return "DATABASE_URL";
  if (/database_uri/.test(lower)) return "DATABASE_URI";
  if (/datasource_url/.test(lower)) return "DATASOURCE_URL";
  if (/db_url/.test(lower)) return "DB_URL";
  if (/sqlalchemy_database_uri/.test(lower)) return "SQLALCHEMY_DATABASE_URI";
  if (/connectionstrings/.test(lower)) return "ConnectionStrings";
  if (/jdbc:/.test(lower)) return "JDBC";
  return null;
}

export function hasDatabaseRuntimeSignal(lower: string): boolean {
  return /database_url|database_uri|db_url|spring[_\.]datasource|datasource[_\.]url|sqlalchemy_database_uri|connectionstrings|jdbc:/.test(lower);
}

export function persistenceKindFromSelection(input: {
  status: string | null;
  selection: string | null;
  normalizedSelection: string | null;
}): DependencyServiceKind | "sqlite" | "file" | null {
  const value = input.normalizedSelection ?? "";
  if (/not_needed|not_applicable/i.test(input.status ?? "") || /no persistence|不需要|none|not needed/.test(value)) {
    return null;
  }
  if (/postgres|postgresql|pgsql/.test(value)) return "postgres";
  if (/mysql|mariadb/.test(value)) return "mysql";
  if (/mongodb|mongo/.test(value)) return "mongodb";
  if (/sqlite/.test(value)) return "sqlite";
  if (/file|json/.test(value)) return "file";
  return null;
}

export function persistenceServiceKindFromSelection(input: {
  status: string | null;
  selection: string | null;
  normalizedSelection: string | null;
}): DependencyServiceKind | null {
  const kind = persistenceKindFromSelection(input);
  return isDatabaseServiceKind(kind) ? kind : null;
}

export function isDatabaseServiceKind(kind: unknown): kind is DependencyServiceKind {
  return typeof kind === "string" && DATABASE_SERVICE_KINDS.has(kind as DependencyServiceKind);
}

export function isSqlServiceKind(kind: unknown): kind is "postgres" | "mysql" {
  return typeof kind === "string" && SQL_SERVICE_KINDS.has(kind as DependencyServiceKind);
}

export function isDependencyManifestPath(pathValue: string): boolean {
  return dependencyManifestPattern.test(pathValue);
}

export function serviceDefinition(kind: DependencyServiceKind, reason: string): DependencyService {
  switch (kind) {
    case "postgres":
      return {
        kind,
        serviceName: "postgres",
        image: "postgres:16-alpine",
        port: 5432,
        env: {
          POSTGRES_USER: "loom",
          POSTGRES_PASSWORD: "loom",
          POSTGRES_DB: "loom",
        },
        connectionEnv: {
          DATABASE_URL: "postgresql://loom:loom@postgres:5432/loom",
        },
        volumeName: "postgres-data",
        volumeTarget: "/var/lib/postgresql/data",
        reason,
      };
    case "redis":
      return {
        kind,
        serviceName: "redis",
        image: "redis:7-alpine",
        port: 6379,
        env: {},
        connectionEnv: {
          REDIS_URL: "redis://redis:6379",
        },
        volumeName: "redis-data",
        volumeTarget: "/data",
        reason,
      };
    case "mysql":
      return {
        kind,
        serviceName: "mysql",
        image: "mysql:8",
        port: 3306,
        env: {
          MYSQL_ROOT_PASSWORD: "loom",
          MYSQL_DATABASE: "loom",
          MYSQL_USER: "loom",
          MYSQL_PASSWORD: "loom",
        },
        connectionEnv: {
          DATABASE_URL: "mysql://loom:loom@mysql:3306/loom",
        },
        volumeName: "mysql-data",
        volumeTarget: "/var/lib/mysql",
        reason,
      };
    case "mongodb":
      return {
        kind,
        serviceName: "mongodb",
        image: "mongo:7",
        port: 27017,
        env: {
          MONGO_INITDB_ROOT_USERNAME: "loom",
          MONGO_INITDB_ROOT_PASSWORD: "loom",
        },
        connectionEnv: {
          MONGODB_URL: "mongodb://loom:loom@mongodb:27017/loom?authSource=admin",
        },
        volumeName: "mongodb-data",
        volumeTarget: "/data/db",
        reason,
      };
    case "rabbitmq":
      return {
        kind,
        serviceName: "rabbitmq",
        image: "rabbitmq:3-alpine",
        port: 5672,
        env: {
          RABBITMQ_DEFAULT_USER: "loom",
          RABBITMQ_DEFAULT_PASS: "loom",
        },
        connectionEnv: {
          RABBITMQ_URL: "amqp://loom:loom@rabbitmq:5672",
        },
        volumeName: "rabbitmq-data",
        volumeTarget: "/var/lib/rabbitmq",
        reason,
      };
    case "elasticsearch":
      return {
        kind,
        serviceName: "elasticsearch",
        image: "docker.elastic.co/elasticsearch/elasticsearch:8.15.3",
        port: 9200,
        env: {
          discovery_type: "single-node",
          xpack_security_enabled: "false",
          ES_JAVA_OPTS: "-Xms512m -Xmx512m",
        },
        connectionEnv: {
          ELASTICSEARCH_URL: "http://elasticsearch:9200",
        },
        volumeName: "elasticsearch-data",
        volumeTarget: "/usr/share/elasticsearch/data",
        reason,
      };
    case "minio":
      return {
        kind,
        serviceName: "minio",
        image: "minio/minio:RELEASE.2025-04-22T22-12-26Z",
        port: 9000,
        env: {
          MINIO_ROOT_USER: "loom",
          MINIO_ROOT_PASSWORD: "loom-password",
        },
        connectionEnv: {
          S3_ENDPOINT: "http://minio:9000",
          S3_ACCESS_KEY_ID: "loom",
          S3_SECRET_ACCESS_KEY: "loom-password",
          S3_BUCKET: "loom",
        },
        volumeName: "minio-data",
        volumeTarget: "/data",
        reason,
      };
  }
}

export function dedupeDependencyServices<T extends { kind: DependencyServiceKind }>(services: T[]): T[] {
  const seen = new Set<DependencyServiceKind>();
  return services.filter((service) => {
    if (seen.has(service.kind)) {
      return false;
    }
    seen.add(service.kind);
    return true;
  });
}

export function normalizeDependencyConnectionEnv(services: DependencyService[]): DependencyService[] {
  const sqlServices = services.filter((service) => isSqlServiceKind(service.kind));
  if (sqlServices.length <= 1) {
    return services;
  }

  return services.map((service) => {
    if (service.kind === "postgres") {
      return {
        ...service,
        connectionEnv: {
          POSTGRES_URL: "postgresql://loom:loom@postgres:5432/loom",
        },
        reason: `${service.reason} Multiple SQL services detected, so DATABASE_URL was not assigned automatically.`,
      };
    }
    if (service.kind === "mysql") {
      return {
        ...service,
        connectionEnv: {
          MYSQL_URL: "mysql://loom:loom@mysql:3306/loom",
        },
        reason: `${service.reason} Multiple SQL services detected, so DATABASE_URL was not assigned automatically.`,
      };
    }
    return service;
  });
}

export function springDatasourceEnv(kind: "postgres" | "mysql"): Record<string, string> {
  if (kind === "postgres") {
    return {
      DATABASE_URL: "postgresql://loom:loom@postgres:5432/loom",
      SPRING_DATASOURCE_URL: "jdbc:postgresql://postgres:5432/loom",
      SPRING_DATASOURCE_USERNAME: "loom",
      SPRING_DATASOURCE_PASSWORD: "loom",
    };
  }
  return {
    DATABASE_URL: "mysql://loom:loom@mysql:3306/loom",
    SPRING_DATASOURCE_URL: "jdbc:mysql://mysql:3306/loom",
    SPRING_DATASOURCE_USERNAME: "loom",
    SPRING_DATASOURCE_PASSWORD: "loom",
  };
}

export function withSpringDatasourceConnectionEnv(
  service: DependencyService,
  stack: DetectedStack,
  evidenceText: string,
): DependencyService {
  const combined = evidenceText.toLowerCase();
  const springLike = /spring[_\.]datasource/.test(combined) ||
    (stack.kind === "java" && (stack.framework === "spring-boot" || /application\.ya?ml|application\.properties/.test(combined)));
  if (!springLike || !isSqlServiceKind(service.kind)) {
    return service;
  }
  return {
    ...service,
    connectionEnv: {
      ...service.connectionEnv,
      ...springDatasourceEnv(service.kind),
    },
  };
}

function dedupeKinds(kinds: DependencyServiceKind[]): DependencyServiceKind[] {
  const seen = new Set<DependencyServiceKind>();
  return kinds.filter((kind) => {
    if (seen.has(kind)) {
      return false;
    }
    seen.add(kind);
    return true;
  });
}

function hasTokenSignal(value: string, token: string): boolean {
  const escaped = token.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`(^|[^a-z0-9_@/-])${escaped}(?=$|[^a-z0-9_@/-])`, "i").test(value);
}
