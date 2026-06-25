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
