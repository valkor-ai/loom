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
