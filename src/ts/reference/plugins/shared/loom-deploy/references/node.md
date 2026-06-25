# Node Deployment Reference

Use this reference when implementing or repairing loom deploy support for Node-family projects.

## Scanner Signals

- `package.json` identifies a Node project.
- Lockfiles select package manager:
  - `pnpm-lock.yaml` -> pnpm
  - `yarn.lock` -> yarn
  - `bun.lock` or `bun.lockb` -> bun
  - `package-lock.json` or no recognized lockfile -> npm
- `scripts.build` becomes the build command.
- `scripts.start` is preferred as the runtime command.
- Next.js `output: "standalone"` in `next.config.*` should prefer `node .next/standalone/server.js` as the runtime command.
- Vite with `scripts.preview` should run preview with host binding, for example `npm run preview -- --host 0.0.0.0`.

## Framework Hints

- `next` dependency -> Next.js, default port 3000.
- `vite` dependency or script -> Vite, default preview port 4173.
- `astro` dependency -> Astro, default port 4321.
- `express`, `fastify`, `koa`, or `hono` dependency -> Node server, default port 3000.
- No start/preview script -> Node CLI/library project. Generate a container that builds and explains no start script was detected, rather than pretending to serve HTTP.

## Template Rules

- Use the project's declared Node major version when detected from `package.json` `engines.node`, `package.json` `volta.node`, `.nvmrc`, `.node-version`, or `.tool-versions`.
- If no project Node version signal exists, default to `node:22-slim`.
- Use a Linux/glibc slim image such as `node:<major>-slim` for Node and Next.js projects. Avoid Alpine for Next/Tailwind/CSS pipelines unless there is a project-specific reason, because native optional dependencies often differ between glibc and musl.
- For Bun projects, prefer `oven/bun:1` instead of installing Bun into a Node image.
- Use `npm ci` only when a package lockfile exists; otherwise use `npm install`.
- For pnpm/yarn, enable Corepack. Respect `packageManager` version metadata when repair work needs to pin package manager versions.
- For Next.js standalone output, run `.next/standalone/server.js` after the production build rather than invoking the package `start` script when the configuration declares standalone output.
- Use Dockerfile-specific ignore files beside generated Dockerfiles, such as `Dockerfile.dockerignore`.
- Do not copy `.env` files into build context by default.
- Generated ignore files should exclude `.next`, `.turbo`, `.vercel`, `out`, `dist`, `build`, and `node_modules` so host-platform build artifacts do not leak into Linux images.

## Platform Awareness

- Containers run on Linux, not the developer host OS. Treat OS, CPU, and libc as deployment inputs.
- On Apple Silicon, Docker usually builds Linux arm64 images. Native optional packages may need `linux-arm64-gnu` for Debian/Ubuntu/slim images or `linux-arm64-musl` for Alpine.
- Common Next/Tailwind native optional packages include `@next/swc-*`, `@tailwindcss/oxide-*`, and `lightningcss-*`.
- If a lockfile generated on macOS only contains `darwin-*` optional packages, container builds can fail with missing Linux native modules. Prefer repairing the project lockfile or generated Dockerfile install step so Linux optional dependencies are installed inside the container.
- If logs mention missing `lightningcss.linux-*.node`, prefer a glibc image such as `node:<major>-slim` and ensure `lightningcss-linux-<arch>-gnu` is present. If using Alpine, ensure the `*-musl` variant is present.
- If logs mention missing `@tailwindcss/oxide-linux-*` or `tailwindcss-oxide.linux-*.node`, ensure the matching `@tailwindcss/oxide-linux-<arch>-gnu` package is available for slim/glibc images. If using Alpine, ensure the `*-musl` variant is available.
- If logs mention missing `@next/swc-linux-*`, ensure the corresponding Next SWC optional package is installed in the image and avoid relying on Next runtime downloads from npm.

## Existing Compose Reuse

- Prefer root-level Compose files over generated templates.
- Do not overwrite existing Compose files during `deploy prepare`.
- Infer preview URL from the first simple published port mapping like `8080:80`; use the host side as the local preview port.

## Dependency Services

- Detect Postgres from `pg`, `postgres`, `postgresql`, `prisma`, or `drizzle-orm` signals.
- Detect Redis from `redis`, `ioredis`, `bullmq`, or related queue signals.
- Detect MySQL from `mysql`, `mysql2`, or `mariadb` signals.
- Detect MongoDB from `mongodb` or `mongoose`.
- Detect RabbitMQ from `rabbitmq`, `amqplib`, or `amqp`.
- Detect Elasticsearch/OpenSearch from `elasticsearch`, `@elastic/elasticsearch`, or `opensearch`.
- Detect MinIO/S3-compatible storage from `minio` or S3 endpoint signals.
- Generated Compose should add dependency services only for generated deployments, not overwrite existing Compose files.
- Dependency services should use Compose internal networking with `expose`, not host `ports`, to avoid local port conflicts.
- If only one SQL service is detected, assign `DATABASE_URL`.
- If both Postgres and MySQL are detected, avoid ambiguous `DATABASE_URL`; assign `POSTGRES_URL` and `MYSQL_URL` instead.
