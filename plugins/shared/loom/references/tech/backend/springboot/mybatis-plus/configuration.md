# MyBatis-Plus Configuration

## Dependency And Scanning

- Use the starter and MyBatis-Plus version compatible with the accepted Spring Boot major version and the repository's build file.
- Reuse the existing dependency-management and package layout. Do not add both Boot 2 and Boot 3 starters.
- Confirm `@MapperScan`, mapper interfaces, XML locations, and the active `SqlSessionFactory` before changing configuration.
- A mapper must extend the project's chosen base abstraction, normally `BaseMapper<Entity>`, when that is the repository convention.
- Do not introduce a second Mapper or Service style in the same module without a task-owned migration reason.

## Interceptor Registration

Register `MybatisPlusInterceptor` on every `SqlSessionFactory` that uses the capability. Keep provider-specific settings, such as pagination database type, in the selected persistence configuration.

Verify interceptor order when combining pagination, tenant, data permission, dynamic table, optimistic locking, illegal SQL, and block-attack interceptors. Do not assume a plugin registered on one factory protects another.

## Safe Defaults

- Bind related settings through typed configuration where the repository already does so.
- Keep credentials and production endpoints outside committed configuration.
- Do not enable automatic schema mutation as a substitute for the accepted migration tool.
- Do not treat mapper scanning or a plugin registration as a permission or tenant boundary.

## Verification Focus

Check dependency resolution, mapper discovery, XML namespace/path alignment, interceptor registration, provider configuration, and startup behavior for the active profile.
