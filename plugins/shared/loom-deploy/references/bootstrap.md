# Deployment Bootstrap Reference

Use this reference when deployment diagnostics mention missing tables, pending migrations, schema setup, Prisma, Django, Rails, Laravel, Flyway, Liquibase, or `loom deploy bootstrap`.

## Detection

Bootstrap tasks are advisory diagnostics recorded in `DeploymentSpec.bootstrap.tasks`.

Common tasks:

- Prisma: `npx prisma migrate deploy`, `pnpm exec prisma migrate deploy`, `yarn prisma migrate deploy`, or `bunx prisma migrate deploy`
- Django: `python manage.py migrate --noinput`
- Rails: `bundle exec rails db:migrate`
- Laravel: `php artisan migrate --force`
- Flyway: `flyway migrate`
- Liquibase: `liquibase update`

## Execution Rules

- `loom deploy bootstrap --project-root /abs/project` only previews detected tasks.
- Execute bootstrap commands only when the user explicitly approves and `--confirm` is present.
- Execute against the active local Compose app service with `docker compose exec -T <app-service> sh -lc <command>`.
- Do not run bootstrap commands when the deployment is not running.
- If more than one task is detected, prefer `--kind <kind>` when the user approves a specific migration system.
- Stop after the first failed bootstrap command and return stdout/stderr tails.

## Safety

- Treat migrations as stateful operations against the local Compose dependency services.
- Do not run destructive reset/seed/drop commands automatically.
- Do not read or inject real `.env` values. Use generated local dependency env already in Compose.
- If bootstrap needs credentials or private network access, ask the user for a safe local configuration instead of inventing values.
