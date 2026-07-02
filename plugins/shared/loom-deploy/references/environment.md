# Environment Diagnostics Reference

Use this reference when implementing or repairing loom deploy behavior related to environment variables, secrets, framework config, or generated Compose `environment`.

## Scanner Rules

Record variable names from:

- `.env.example`, `.env.sample`, `.env.local.example`, `.env.template`, and `.env.dist`
- local `.env`, `.env.local`, `.env.development`, and `.env.production` names only
- source-code references such as `process.env.X`, `import.meta.env.X`, `os.getenv("X")`, `System.getenv("X")`, `Environment.GetEnvironmentVariable("X")`, `getenv("X")`, and `ENV["X"]`
- framework-required variables such as Laravel `APP_KEY`, Rails `SECRET_KEY_BASE`, Django `SECRET_KEY`, and NextAuth `NEXTAUTH_SECRET`

Do not read, print, copy, or inject real local `.env` values. Local `.env` files only prove that a variable name exists on the developer machine.

## Required vs Optional

Treat obvious runtime defaults as optional when loom generates them, such as `PORT`, `NODE_ENV`, `RAILS_ENV`, `RACK_ENV`, `SERVER_PORT`, and `ASPNETCORE_URLS`.

Treat public frontend env names such as `NEXT_PUBLIC_*`, `VITE_*`, and `PUBLIC_*` as referenced but not required for boot unless logs prove otherwise.

Treat secrets, tokens, passwords, keys, JWT/session/cookie variables, and connection URLs as required when referenced by examples or source code unless loom already generated a safe local default.

## Generated Defaults

Generated Compose may include:

- runtime defaults such as `PORT`
- dependency service connection values such as `DATABASE_URL`, `REDIS_URL`, `MONGODB_URL`, and related service URLs
- local-only placeholders for common framework secrets where they are needed to boot a local preview
- container-safe file database URLs when the project already points at local file databases such as SQLite, H2 file, HSQLDB file, or Derby file
- framework override variables that make the generated local container agree with the generated runtime, such as `SERVER_PORT`, `ASPNETCORE_URLS`, or safe local profile flags

Generated placeholders are not production secrets. They exist only to make local deployment diagnosable and runnable.

## File Databases And Local State

For local file databases, container paths must be inside a mounted writable directory, for example `/app/data`. Compose should create a named volume or project-local generated volume for that directory. Do not point a container at a host-only relative path that existed only on the developer machine.

For Spring Boot plus JPA/Flyway/Liquibase style stacks, do not assume Hibernate schema validation is authoritative for all local file databases. If generated deployment is supplying a containerized file database URL and migration tooling owns schema creation, prefer a safe local override that prevents schema validation from failing on SQLite/H2 type affinity before the app can boot.

When dependency services are generated, application URLs must use Compose service names such as `postgres`, `mysql`, or `redis`, not `localhost`. Browser-facing frontend env may use public proxy paths; container-to-container env must use service DNS names.

## Repair Guidance

When `environment.missing` is non-empty, inspect it before editing Dockerfile/Compose. If the missing variable can be safely generated for local deployment, add it to generated Compose only. If it is a real credential, ask the user for a safe local value or explain the blocker.

If logs mention missing env, missing secret, invalid config, app key, secret key base, database URL, auth secret, JWT secret, or credentials, compare the log with `DeploymentSpec.environment` and update generated deployment files or ask for user-provided values.

If logs mention missing tables, pending migrations, schema drift, Prisma migration errors, Django/Rails/Laravel migration errors, Flyway, or Liquibase, compare the log with `DeploymentSpec.bootstrap`. Treat bootstrap commands as diagnostic guidance only; ask before running them.

Do not turn every boot error into a user confirmation. If the failure can be fixed inside generated Compose/Dockerfile with safe local defaults and the affected files are editable, repair the generated deployment assets. Ask the user only for real credentials, destructive state changes, or edits to protected user-owned assets.
