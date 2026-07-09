# Go Deployment Reference

Use this reference when implementing or repairing loom deploy support for Go projects.

## Scanner Signals

- `go.mod` identifies a Go project.
- `go.sum` means dependency checksums are present.
- Framework hints:
  - `github.com/gin-gonic/gin` -> Gin.
  - `github.com/labstack/echo` -> Echo.
  - `github.com/gofiber/fiber` -> Fiber.
- Port detection reads simple `PORT=9090` or `port: 9090` signals from project metadata and env examples; otherwise default to 8080.

## Template Rules

- Use a multi-stage Dockerfile.
- Build with `golang:1.23-alpine`.
- Run from `alpine:3.20`.
- Build command: `CGO_ENABLED=0 GOOS=linux go build -o /out/server .`.
- Runtime command: `/app/server`.

## Repair Notes

- Common failures are module download errors, packages that require CGO/system libraries, multi-command repos where the entrypoint is under `./cmd/<name>`, or applications that do not bind to `0.0.0.0`.
- Keep repairs in generated deployment files unless the user approves app source or module changes.

## Scanner Signals To Deploy Facts

Translate Go scanner evidence into deploy facts before generating files:

- `go.mod` path becomes the module root, service root, and manifest ref.
- `go.sum` becomes the lock/checksum ref used before source copy.
- `main` packages under root or `cmd/*` become entrypoint candidates.
- Framework/import signals such as Gin, Echo, Fiber, Chi, net/http, or gRPC decide whether the service is HTTP/API.
- Port/env examples, `os.Getenv("PORT")`, router listen calls, and docs become runtime port facts.
- SQL/Redis/Mongo/RabbitMQ/cloud SDK imports and env names become dependency service facts.
- Non-HTTP command packages become command-style deploy facts without invented preview routes.

## Generated Asset Expectations

Generated Go assets should show:

- Multi-stage build with module download before source copy.
- Build command targeting the selected `main` package, for example `.` or `./cmd/server`.
- Static binary output copied into a slim runtime image.
- `CGO_ENABLED=0` only when dependency facts do not require CGO. If CGO is required, use a runtime/build image with matching system libraries.
- Runtime command executes the generated binary and exposes the selected container port.
- Compose env includes `PORT` only when the app reads it or framework defaults need it.
- Dependency URLs use Compose service DNS names.

## Repair Boundary

Repair generated Go deploy assets when:

- Build target points at the wrong `main` package.
- `CGO_ENABLED=0` conflicts with required CGO/native dependencies.
- Dockerfile context omits `go.mod`, `go.sum`, or internal packages.
- Runtime image misses required certificates, timezone data, or native libraries.
- App listens on localhost or wrong port through generated command/env.

Do not change Go source, module paths, or generated code during deploy asset repair unless the MCP action routes to execution repair.
