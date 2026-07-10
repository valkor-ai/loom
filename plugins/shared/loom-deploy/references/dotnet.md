# .NET Deployment Reference

Use this reference when implementing or repairing loom deploy support for .NET projects.

## Scanner Signals

- A root-level `*.csproj` or `*.sln` identifies a .NET project.
- `Microsoft.NET.Sdk.Web`, `Microsoft.AspNetCore`, or ASP.NET app builder code signals ASP.NET Core.
- `TargetFramework` or `TargetFrameworks` selects the .NET major version, for example `net8.0` -> `8`.
- `global.json` SDK version can be used as a fallback runtime major version signal.
- `ASPNETCORE_URLS`, launch settings, or generic `PORT` signals may identify the runtime port. Default ASP.NET Core local container port is 8080.

## Template Rules

- Use a multi-stage Dockerfile.
- Build with `mcr.microsoft.com/dotnet/sdk:<major>`.
- Run ASP.NET Core projects with `mcr.microsoft.com/dotnet/aspnet:<major>`.
- Run non-web .NET projects with `mcr.microsoft.com/dotnet/runtime:<major>`.
- Use `dotnet restore`, then `dotnet publish -c Release -o /app/publish --no-restore`.
- Set `ASPNETCORE_URLS=http://0.0.0.0:<port>` and `PORT=<port>` for generated Compose/runtime environment.
- Run the published assembly with `dotnet /app/<ProjectName>.dll`.

## Dependency Services

- Detect Postgres from `Npgsql`, `postgres`, or postgres connection strings.
- Detect MySQL/MariaDB from `MySqlConnector`, `mysql`, or MariaDB connection strings.
- Detect Redis from `StackExchange.Redis` or Redis connection strings.
- Detect MongoDB from `MongoDB.Driver`.
- Detect RabbitMQ from `RabbitMQ.Client`.
- Detect Elasticsearch/OpenSearch from Elastic client packages.

## Repair Notes

- If publish succeeds but the runtime cannot find the DLL, inspect the project file name and published output; the start command should match the assembly name.
- If healthcheck fails, verify `ASPNETCORE_URLS`, app `urls` config, and whether HTTPS redirection is forcing an HTTPS-only endpoint.
- If restore fails for private feeds, ask for NuGet credentials or a project-specific `NuGet.Config` rather than baking secrets into generated files.
- If the project has only a `.sln` and multiple web projects, a coding agent should inspect the solution and pick the intended startup project before editing generated deployment files.

## Scanner Signals To Deploy Facts

Translate .NET scanner evidence into deploy facts before generating files:

- `*.csproj` path becomes service root and manifest ref. A `.sln` becomes workspace context when multiple projects are involved.
- `Microsoft.NET.Sdk.Web`, ASP.NET packages, minimal API builder code, controllers, or `UseRouting` decide HTTP/API service facts.
- `TargetFramework`, `TargetFrameworks`, and `global.json` select SDK/runtime image majors.
- `launchSettings.json`, `ASPNETCORE_URLS`, `urls`, Kestrel config, and docs become runtime port facts.
- EF Core packages/migrations, connection strings, Redis/Mongo/RabbitMQ packages, and env examples become dependency service facts.
- Non-web worker/service projects become command-style deploy facts without fake preview routes.

## Generated Asset Expectations

Generated .NET assets should show:

- SDK build stage and ASP.NET/runtime final stage selected by project type.
- `dotnet restore` and `dotnet publish` against the selected project file, not a random solution member.
- Runtime command points to the published assembly name from the selected project.
- `ASPNETCORE_URLS=http://0.0.0.0:<port>` and `PORT=<port>` for web services.
- Compose dependencies mapped into connection strings using service DNS names.
- HTTPS redirection does not make the local HTTP preview unreachable.

## Repair Boundary

Repair generated .NET deploy assets when:

- Restore/publish uses the wrong project path.
- Runtime command references the wrong DLL.
- SDK/runtime image major mismatches target framework.
- `ASPNETCORE_URLS` or port wiring makes healthcheck unreachable.
- Generated connection strings point at localhost or omit generated dependency credentials.

Do not edit C# source, project files, migrations, or NuGet feeds during deploy asset repair unless the MCP action routes to execution repair or credentials are provided.
