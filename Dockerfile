FROM alpine:latest

EXPOSE 8000

VOLUME ["/app/data"]

COPY templates /app/templates
COPY Rocket.toml /app/Rocket.toml
COPY target/x86_64-unknown-linux-musl/release/file-host /app/file-host

ENV ROCKET_ENV=production
WORKDIR "/app/"
ENTRYPOINT ["/app/file-host", "/app/data"]
