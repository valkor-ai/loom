# Deployment Bootstrap Reference

Use this reference when deployment diagnostics mention missing tables, pending migrations, schema setup, Prisma, Django, Rails, Laravel, Flyway, Liquibase, or `loom.deployBootstrap`.

## Detection

Bootstrap tasks are advisory diagnostics recorded in `DeploymentSpec.bootstrap.tasks`.
This list documents the task types currently emitted by Loom's deploy bootstrap scanner. It is not a general migration catalog.

Detected task types:

- Prisma: emitted when `prisma/schema.prisma` exists or package scripts mention Prisma migration commands. The command is selected from the detected package manager: `npx prisma migrate deploy`, `pnpm exec prisma migrate deploy`, `yarn prisma migrate deploy`, or `bunx prisma migrate deploy`.
- Django: emitted when `manage.py` is detected. Command: `python manage.py migrate --noinput`.
- Rails: emitted when `db/migrate` is detected. Command: `bundle exec rails db:migrate`.
- Laravel: emitted when `database/migrations` is detected. Command: `php artisan migrate --force`.
- Flyway: emitted when `flyway.conf`, `flyway.toml`, or `src/main/resources/db/migration` is detected. Java projects may use Maven or Gradle Flyway plugin commands; otherwise command is `flyway migrate`.
- Liquibase: emitted when `liquibase.properties`, `liquibase.yml`, `liquibase.yaml`, or `src/main/resources/db/changelog` is detected. Java projects may use Maven or Gradle Liquibase plugin commands; otherwise command is `liquibase update`.

Agent boundary:

- Do not invent bootstrap commands that are not present in `DeploymentSpec.bootstrap.tasks`.
- If the repository uses a migration system not listed here, treat it as unsupported by the current scanner unless Loom has already emitted a task for it.
- Repair may improve scanner support in MCP code, but deploy execution must only run declared tasks.

## Execution Contract

- `loom.deployBootstrap` with `confirm: false` previews detected tasks and returns a user gate. It must not execute commands.
- `loom.deployBootstrap` with `confirm: true` may execute only tasks from `DeploymentSpec.bootstrap.tasks`.
- The MCP tool executes confirmed tasks inside the active Compose primary app service with `docker compose exec -T <service> sh -lc <command>`.
- The deployment must already be running. If the Compose service is not running, use `loom.deployUp` before retrying bootstrap.
- If more than one task is detected, pass `kind` when the user approves a specific migration system.
- Stop after the first failed bootstrap command. Report the task kind, command, Compose path, service id, exit code, and stdout/stderr tails returned by the tool.

## Agent Boundary

- Do not hand-run migration commands in a local shell.
- Do not edit generated Compose or Dockerfile assets just to make bootstrap commands run.
- Do not invent extra bootstrap tasks. Use only tasks declared by Loom.
- If the tool reports that the service is not running, continue through the recommended Loom deploy action instead of patching files.

## Safety

- Treat migrations as stateful operations against the local Compose dependency services.
- Do not run destructive reset/seed/drop commands automatically.
- Do not read or inject real `.env` values. Use generated local dependency env already in Compose.
- If bootstrap needs credentials or private network access, ask the user for a safe local configuration instead of inventing values.
