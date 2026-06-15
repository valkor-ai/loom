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
