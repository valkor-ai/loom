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
- Use maintained Eclipse Temurin images:
  - Maven builder: `maven:3-eclipse-temurin-<major>`.
  - Gradle builder: `gradle:8-jdk<major>`.
  - Runtime: `eclipse-temurin:<major>-jre`.
- Default Java major version is 21 when the project does not declare one.
- Copy the first runnable jar from `target` or `build/libs`, excluding `*-plain.jar`, `*-sources.jar`, and `*-javadoc.jar`.
- Set both `PORT` and `SERVER_PORT` in generated Compose/runtime environment for Spring Boot compatibility.

## Dependency Services

- Detect Postgres from JDBC URLs, `postgresql`, `org.postgresql`, Flyway/Liquibase migration config, or Spring datasource settings.
- Detect MySQL/MariaDB from JDBC URLs, `mysql`, `mariadb`, or driver dependencies.
- Detect Redis from `spring-data-redis`, `lettuce`, or `jedis`.
- Detect MongoDB from `mongodb` or Spring Data MongoDB.
- Detect RabbitMQ from `amqp`, `spring-rabbit`, or RabbitMQ config.
- Detect Elasticsearch/OpenSearch from client dependencies or endpoint variables.

## Repair Notes

- If the build cannot find a wrapper script, fall back to the installed Maven/Gradle command in the builder image.
- If the final jar cannot be found, inspect the build output directory and exclude classifier jars before selecting the application jar.
- If a Spring Boot container starts but healthcheck fails, verify `SERVER_PORT`, `server.address`, profile-specific config, and whether the app requires database migrations or secrets.
- If Gradle builds fail due to daemon or cache issues, disable the daemon or rerun with a clean generated image before changing application code.
