# MyBatis-Plus Reference Routing

This profile applies only when the accepted TechnicalBaseline selects MyBatis-Plus, or an existing-project scan has high-confidence MyBatis-Plus evidence such as `com.baomidou.mybatisplus`, `mybatis-plus-boot-starter`, `BaseMapper`, or `MybatisPlusInterceptor`.

Do not apply it to MyBatis-Flex, plain MyBatis, JPA, Hibernate, Spring Data, or an arbitrary class named `Mapper`.

## Route By Ownership

| Task-owned capability | Read |
|---|---|
| Starter, mapper scanning, global configuration | `configuration.md` |
| Entity mapping, IDs, logical deletion, optimistic locking, enum/JSON fields, audit fill | `mapping.md` |
| Mapper, Service, CRUD, batch, paging, stream query | `crud.md` |
| Query or update wrappers and partial updates | `wrappers.md` |
| Pagination, tenant, lock, dynamic table, block-attack interceptors | `plugins.md` |
| SQL fragments, user-controlled sorting, injection risk | `security.md` |
| Generator, ID generator, SQL injector, ActiveRecord, Db Kit, multiple data sources, DDL | `extensions.md` |

Generic Spring Boot runtime, web, security, logging, and testing rules remain in their existing Spring Boot references. SQL dialect behavior remains in the selected `tech/code/sql` references.

## Execution Rules

- Follow the accepted backend and data-access selections; do not choose another ORM during execution.
- Reuse the repository's existing Mapper, Service, transaction, migration, permission, tenant, and audit conventions.
- Keep high-impact extensions task-owned and explicit. Do not add a generator, global interceptor, automatic DDL, or multi-data-source setup as incidental cleanup.
- Record the selected reference files and the affected persistence behavior in the existing code-quality evidence.

## Verification Focus

Prove the task-owned mapping, query/update semantics, transaction boundary, tenant or permission boundary, provider behavior, and failure path. A compile pass alone does not prove SQL, interceptor, or persistence behavior.
