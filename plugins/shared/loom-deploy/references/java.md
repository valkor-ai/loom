# Java Deployment Reference

Use this reference when implementing or repairing loom deploy support for Java-family projects.

## Scanner Signals

- `pom.xml` or `mvnw` identifies a Maven project.
- `build.gradle`, `build.gradle.kts`, `settings.gradle`, `settings.gradle.kts`, or `gradlew` identifies a Gradle project.
- `org.springframework.boot` or `spring-boot` dependency/plugin signals Spring Boot.
- `io.quarkus` signals Quarkus.
- `io.micronaut` signals Micronaut.
- Java version signals may appear in Maven properties such as `java.version`, `maven.compiler.release`, or `maven.compiler.target`; Gradle signals include `sourceCompatibility`, `targetCompatibility`, and toolchain `languageVersion`.
- `server.port` in `application.properties`, or `server: port:` style YAML, should become the runtime port. Default Java web port is 8080.

## Template Rules

- Use a multi-stage Dockerfile.
- Prefer project wrappers when present:
  - Maven: `./mvnw -DskipTests package`, otherwise `mvn -DskipTests package`.
  - Gradle: `./gradlew build -x test`, otherwise `gradle build -x test`.
- Run wrapper commands from the directory that contains the wrapper/build file. If the service root is `service/`, the Dockerfile must `WORKDIR /app/service` before invoking `./gradlew` or `./mvnw`.
- Use maintained Eclipse Temurin images:
  - Maven builder: `maven:3-eclipse-temurin-<major>`.
  - Gradle builder: `gradle:8-jdk<major>`.
  - Runtime: `eclipse-temurin:<major>-jre`.
- Default Java major version is 21 when the project does not declare one.
- Copy the first runnable jar from `target` or `build/libs`, excluding `*-plain.jar`, `*-sources.jar`, and `*-javadoc.jar`.
- Set both `PORT` and `SERVER_PORT` in generated Compose/runtime environment for Spring Boot compatibility.
- If a frontend build output must be served by Spring Boot, copy that output into `src/main/resources/static` or the generated build staging area before packaging the jar; do not run a separate frontend dev server in the runtime image.

## Dependency Services

- Detect Postgres from JDBC URLs, `postgresql`, `org.postgresql`, Flyway/Liquibase migration config, or Spring datasource settings.
- Detect MySQL/MariaDB from JDBC URLs, `mysql`, `mariadb`, or driver dependencies.
- Detect Redis from `spring-data-redis`, `lettuce`, or `jedis`.
- Detect MongoDB from `mongodb` or Spring Data MongoDB.
- Detect RabbitMQ from `amqp`, `spring-rabbit`, or RabbitMQ config.
- Detect Elasticsearch/OpenSearch from client dependencies or endpoint variables.

## Persistence And Migrations

- JDBC URLs using Compose dependency services must use service DNS names, not `localhost`.
- File database URLs such as SQLite, H2 file, HSQLDB file, and Derby file should point at a writable mounted container path such as `/app/data`.
- When Flyway or Liquibase is detected, treat migration tooling as the schema owner for local deployment. Framework schema validation that is known to misread file database type affinity should be disabled or downgraded with a safe local generated env override instead of causing container startup failure.
- Do not assume SQLite for every Java app. Use this path only when repository config or generated env explicitly points at a file database.

## Repair Notes

- If the build cannot find a wrapper script, fall back to the installed Maven/Gradle command in the builder image.
- If the final jar cannot be found, inspect the build output directory and exclude classifier jars before selecting the application jar.
- If a Spring Boot container starts but healthcheck fails, verify `SERVER_PORT`, `server.address`, profile-specific config, and whether the app requires database migrations or secrets.
- If Gradle builds fail due to daemon or cache issues, disable the daemon or rerun with a clean generated image before changing application code.
- If Java build context misses a sibling frontend or shared module, fix Compose build context and Dockerfile copy paths together.
