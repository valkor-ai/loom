# PHP Deployment Reference

Use this reference when implementing or repairing loom deploy support for PHP-family projects.

## Scanner Signals

- `composer.json` identifies a PHP project and should be checked before `package.json`, because Laravel projects commonly include frontend assets.
- `laravel/framework` or `artisan` signals Laravel.
- `symfony/framework-bundle` or `symfony/runtime` signals Symfony.
- `slim/slim` signals Slim.
- `composer.lock` is the Composer lockfile.
- `composer.json` `require.php` should guide the PHP minor version when possible. Default PHP version is 8.3.

## Template Rules

- Use a single-container local preview template for v1.
- Use `php:<minor>-cli` plus Composer for deterministic local preview.
- Install common extensions needed by web apps: `pdo`, `pdo_mysql`, `pdo_pgsql`, and `zip`.
- Copy `composer.json` and `composer.lock` before source files, then run Composer install.
- For Laravel, create `storage` and `bootstrap/cache`, then run `php artisan package:discover --ansi || true`.
- Use `php artisan serve --host=0.0.0.0 --port=${PORT:-8000}` for Laravel local preview.
- For generic PHP, use the built-in server with `public/index.php`.

## Dependency Services

- Detect MySQL/MariaDB from `pdo_mysql`, `mysqli`, `mysql`, or Laravel database config.
- Detect Postgres from `pdo_pgsql`, `pgsql`, or postgres connection strings.
- Detect Redis from `predis`, `phpredis`, or Redis connection strings.
- Detect RabbitMQ, Elasticsearch, MongoDB, and S3-compatible services from Composer package names and env/config signals.

## Repair Notes

- If Composer install fails on missing PHP extensions, update the generated Dockerfile extension install block before editing application code.
- If Laravel starts but returns a 500, inspect logs for missing `APP_KEY`, write permissions in `storage`, database migration failures, or missing env values.
- Do not copy real `.env` files into generated images by default; use `.env.example` to infer needed variables.
- For production-grade PHP deployments, a future provider may use Nginx + PHP-FPM, but the v1 Dockerfile template is intentionally a local preview path.

## Scanner Signals To Deploy Facts

Translate PHP scanner evidence into deploy facts before generating files:

- `composer.json` path becomes service root, manifest ref, and Composer install fact.
- `composer.lock` becomes lockfile ref.
- Laravel `artisan`, Symfony runtime/config, Slim route setup, or `public/index.php` decide framework/runtime facts.
- `composer.json` `require.php` selects PHP image version when possible.
- `public/` document root, `artisan serve`, and framework router signals become preview/start command facts.
- `.env.example`, config files, DB drivers, queue/cache packages, and storage paths become environment/dependency facts.
- Frontend asset `package.json` inside Laravel/Symfony does not override the PHP app role.

## Generated Asset Expectations

Generated PHP assets should show:

- Composer install before source copy when lockfiles/manifests allow layer caching.
- Required PHP extensions installed according to dependency facts, not a hard-coded database guess.
- Laravel local preview includes generated `APP_KEY`, writable `storage` and `bootstrap/cache`, and a container-safe port.
- Generic PHP/Slim/Symfony preview serves the detected public document root.
- Dependency URLs/config point at Compose service DNS names and generated local credentials.
- Real `.env` values are never copied into the image.

## Repair Boundary

Repair generated PHP deploy assets when:

- Composer install fails because generated extension packages are incomplete.
- The runtime command serves the wrong document root or binds the wrong host/port.
- Laravel/Symfony local secrets/cache/storage defaults are missing from generated env or directories.
- Dependency config uses localhost inside the container.

Do not edit PHP application config, migrations, Composer requirements, or real env files during deploy asset repair unless the MCP action routes to execution repair.
