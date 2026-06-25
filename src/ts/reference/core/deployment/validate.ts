const fatalLogPatterns = [
  /cannot find module/i,
  /module_not_found/i,
  /(@next\/swc|@tailwindcss\/oxide|tailwindcss-oxide|lightningcss|sharp|esbuild|rollup).*?(linux|darwin|arm64|x64|gnu|musl)/i,
  /error:\s+listen/i,
  /eaddrinuse/i,
  /address already in use/i,
  /port is already allocated/i,
  /uncaught exception/i,
  /syntaxerror/i,
  /npm err!/i,
  /npm error/i,
  /missing script:/i,
  /command failed/i,
  /connection refused/i,
  /econnrefused/i,
  /password authentication failed/i,
  /access denied for user/i,
  /relation .* does not exist/i,
  /table .* doesn't exist/i,
  /no such table/i,
  /pending migrations/i,
  /pendingmigrationerror/i,
  /application failed to start/i,
  /beancreationexception/i,
  /unsatisfieddependencyexception/i,
  /applicationcontextexception/i,
  /webserverexception/i,
  /flywayexception/i,
  /liquibaseexception/i,
  /hibernateexception/i,
  /schemamanagementexception/i,
  /psqlexception/i,
  /communications link failure/i,
  /unable to obtain jdbc connection/i,
  /prisma.*p20\d{2}/i,
  /django\.db\.utils\./i,
  /improperlyconfigured/i,
  /active(record|model)::/i,
  /illuminate\\database/i,
  /sqlstate\[/i,
  /permission denied/i,
  /eacces/i,
  /required environment/i,
  /environment variable .* (is not set|missing|required)/i,
  /secret.*(missing|not set)/i,
];

export type LogValidationResult = {
  ok: boolean;
  matchedPattern: string | null;
  lines: string[];
};

export function validateStartupLogs(rawLogs: string): LogValidationResult {
  const lines = rawLogs
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.length > 0)
    .slice(-120);

  for (const line of lines) {
    const matched = fatalLogPatterns.find((pattern) => pattern.test(line));
    if (matched) {
      return {
        ok: false,
        matchedPattern: matched.source,
        lines,
      };
    }
  }

  return {
    ok: true,
    matchedPattern: null,
    lines,
  };
}
