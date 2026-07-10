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

## Environment Fact Flow

Scanner evidence becomes deploy facts before assets are generated:

- Env example names become `environment.expectedNames`.
- Source-code env references become `environment.referencedNames`.
- Known safe local defaults become generated Compose values.
- Real secret names remain diagnostics and must not be filled from local files.
- Dependency facts decide connection URL shapes and service names.
- File database facts decide writable container paths and volume mounts.

Generated Compose should only include values supported by those facts. If a variable is absent from facts and not required by the selected runtime template, do not invent it.

## File Databases And Local State

For local file databases, container paths must be inside a mounted writable directory, for example `/app/data`. Compose should create a named volume or project-local generated volume for that directory. Do not point a container at a host-only relative path that existed only on the developer machine.

For Spring Boot plus JPA/Flyway/Liquibase style stacks, do not assume Hibernate schema validation is authoritative for all local file databases. If generated deployment is supplying a containerized file database URL and migration tooling owns schema creation, prefer a safe local override that prevents schema validation from failing on SQLite/H2 type affinity before the app can boot.

When dependency services are generated, application URLs must use Compose service names such as `postgres`, `mysql`, or `redis`, not `localhost`. Browser-facing frontend env may use public proxy paths; container-to-container env must use service DNS names.

File database handling is not SQLite-specific:

- SQLite examples commonly use `jdbc:sqlite:/app/data/app.db`, `sqlite:////app/data/app.db`, or framework-specific file paths.
- H2 file, HSQLDB file, Derby, LiteFS-backed SQLite, and similar local file stores still need writable mounted paths.
- If the app config names a host path such as `./data/app.db`, translate it into a container path and mount a volume at the parent directory.
- If migrations are present, let migration tooling initialize schema for local deployment unless repository config explicitly disables it.

## Service Dependency URLs

Generate dependency URLs from service facts:

- Postgres: host `postgres`, port `5432`, generated local user/password/database.
- MySQL/MariaDB: host `mysql` or `mariadb`, port `3306`, generated local user/password/database.
- Redis: host `redis`, port `6379`.
- MongoDB: host `mongo` or `mongodb`, port `27017`.
- RabbitMQ: host `rabbitmq`, ports stay internal unless explicitly public.
- MinIO/S3-compatible: endpoint uses the Compose service DNS name and internal port.

Framework-specific variable names can wrap the same service URL. Use the framework's expected config names when detected, but keep the underlying host/port consistent with Compose.

## Framework Local Safety Defaults

Safe local defaults can unblock local preview without pretending to be production configuration:

- Spring Boot: `SERVER_PORT`, local datasource URL/driver when generated, and migration/JPA flags needed for containerized local boot.
- Django: `SECRET_KEY`, `DEBUG=1`, allowed hosts for local container access, and database URL when generated.
- Rails: `SECRET_KEY_BASE`, local database URL, and writable storage/log paths.
- Laravel: `APP_KEY`, `APP_ENV=local`, `APP_DEBUG=true`, storage/cache paths, and generated DB/Redis URLs.
- ASP.NET Core: `ASPNETCORE_URLS`, `ASPNETCORE_ENVIRONMENT=Development`, and connection strings from generated dependencies.
- NextAuth/Auth.js: local `NEXTAUTH_SECRET` or equivalent only when the app requires it to boot.

Do not add framework defaults for a framework that was not detected.

## Repair Guidance

When `environment.missing` is non-empty, inspect it before editing Dockerfile/Compose. If the missing variable can be safely generated for local deployment, add it to generated Compose only. If it is a real credential, ask the user for a safe local value or explain the blocker.

If logs mention missing env, missing secret, invalid config, app key, secret key base, database URL, auth secret, JWT secret, or credentials, compare the log with `DeploymentSpec.environment` and update generated deployment files or ask for user-provided values.

If logs mention missing tables, pending migrations, schema drift, Prisma migration errors, Django/Rails/Laravel migration errors, Flyway, or Liquibase, compare the log with `DeploymentSpec.bootstrap`. Treat bootstrap commands as diagnostic guidance only; ask before running them.

Do not turn every boot error into a user confirmation. If the failure can be fixed inside generated Compose/Dockerfile with safe local defaults and the affected files are editable, repair the generated deployment assets. Ask the user only for real credentials, destructive state changes, or edits to protected user-owned assets.
