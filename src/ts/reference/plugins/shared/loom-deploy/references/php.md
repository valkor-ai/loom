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
