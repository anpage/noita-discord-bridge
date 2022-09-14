FROM docker.io/rust:1.63-alpine3.16 as builder
RUN apk add --no-cache \
    musl-dev
WORKDIR /usr/src/noita-discord-bridge
COPY . .
RUN cargo install --path .

FROM docker.io/alpine:3.16
COPY --from=builder /usr/local/cargo/bin/noita-discord-bridge /usr/local/bin/ndb
EXPOSE 6667
CMD ["ndb"]
