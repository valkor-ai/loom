# Ruby Deployment Reference

Use this reference when implementing or repairing loom deploy support for Ruby-family projects.

## Scanner Signals

- `Gemfile` identifies a Ruby project and should be checked before `package.json`, because Rails projects commonly include frontend assets.
- `rails`, `railties`, or `config/application.rb` signals Rails.
- `sinatra` signals Sinatra.
- `puma` signals a Rack/Ruby web app.
- `Gemfile.lock` is the Bundler lockfile.
- `.ruby-version` or `ruby "x.y.z"` in `Gemfile` should select the Ruby minor version. Default Ruby version is 3.3.

## Template Rules

- Use a single-container local preview template for v1.
- Use `ruby:<minor>-slim`.
- Install common native build dependencies such as `build-essential`, `git`, `libpq-dev`, and `pkg-config`.
- Copy `Gemfile` and `Gemfile.lock` before source files, then run Bundler install.
- For Rails, create `tmp/pids`, `tmp/cache`, `log`, and `storage` directories.
- Use `bundle exec rails server -b 0.0.0.0 -p ${PORT:-3000}` for Rails local preview.
- For Rack/Sinatra apps, use `bundle exec rackup -o 0.0.0.0 -p ${PORT:-3000}`.

## Dependency Services

- Detect Postgres from `pg`, `postgres`, Rails database config, or connection strings.
- Detect Redis from `redis`, `sidekiq`, or Redis connection strings.
- Detect MySQL/MariaDB from `mysql2`, `mysql`, or database config.
- Detect MongoDB from `mongoid` or MongoDB connection strings.
- Detect RabbitMQ and Elasticsearch/OpenSearch from gem names and env/config signals.

## Repair Notes

- If Bundler fails on native extensions, update generated OS package installs before editing app code.
- If Rails boots but returns a 500, inspect logs for missing `SECRET_KEY_BASE`, storage permissions, database connection errors, or pending migrations.
- If assets are required before boot, a future provider should add asset precompile; v1 local preview prioritizes starting the app without forcing production asset compilation.
- Do not copy real `.env` files into generated images by default; use `.env.example` to infer needed variables.

## Scanner Signals To Deploy Facts

Translate Ruby scanner evidence into deploy facts before generating files:

- `Gemfile` path becomes service root, manifest ref, and Bundler install fact.
- `Gemfile.lock` becomes lockfile ref.
- Rails config, `bin/rails`, `config.ru`, Sinatra/Puma gems, and route files decide framework/runtime facts.
- `.ruby-version`, `Gemfile` `ruby` declaration, and lockfile platform hints select Ruby image version and native package needs.
- Rails `database.yml`, ActiveRecord adapters, Redis/Sidekiq, Elasticsearch, and env examples become dependency service facts.
- Node package files inside Rails projects are secondary asset signals and should not override the Ruby app role.

## Generated Asset Expectations

Generated Ruby assets should show:

- Bundler install before source copy when manifests allow caching.
- Native build packages matching detected gems such as `pg`, `mysql2`, `sqlite3`, `nokogiri`, or image processing gems.
- Rails local preview includes generated `SECRET_KEY_BASE`, writable `tmp`, `log`, and `storage` directories.
- Runtime command binds Rails/Rack/Sinatra to `0.0.0.0` and the selected port.
- Dependency URLs/config use Compose service DNS names and generated local credentials.
- Real `.env` values are never copied into the image.

## Repair Boundary

Repair generated Ruby deploy assets when:

- Bundler native extension build fails due to missing OS packages.
- Runtime command points at the wrong Rack/Rails entrypoint or wrong port.
- Rails local secret/storage directories are missing from generated env or filesystem setup.
- Dependency config uses localhost inside the container.

Do not edit Ruby application code, Gemfile dependencies, migrations, or real env files during deploy asset repair unless the MCP action routes to execution repair.
