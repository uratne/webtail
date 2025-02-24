# Server building planner
FROM rust:1.82 AS planner
WORKDIR /build
RUN cargo install cargo-chef
COPY src src
COPY Cargo.toml Cargo.toml
RUN cargo chef prepare --recipe-path recipe.json

# Cache dependencies
FROM rust:1.82 AS cacher
WORKDIR /build
RUN cargo install cargo-chef
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json

# Build server binary
FROM rust:1.82 AS server_builder
WORKDIR /build
COPY src src
COPY Cargo.toml Cargo.toml

# Copy dependencies
COPY --from=cacher /build/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

RUN cargo build --release --bin server -j 8

# Build frontend
FROM node:20-alpine AS frontend_builder
COPY frontend frontend
WORKDIR /frontend
RUN npm install

RUN npm run build

# Run server
FROM ubuntu:24.04 AS runner
WORKDIR /app
COPY --from=server_builder /build/target/release/server /usr/local/bin/webtail
COPY --from=frontend_builder /frontend/build /app/frontend
COPY .prod.env /app/.prod.env

RUN groupadd -r webtail && useradd -r -g webtail webtail
RUN chown -R webtail:webtail /app
RUN chown -R webtail:webtail /usr/local/bin/webtail

ENV ENVIRONMENT=prod

USER webtail:webtail

ENTRYPOINT ["webtail"]

EXPOSE 8080