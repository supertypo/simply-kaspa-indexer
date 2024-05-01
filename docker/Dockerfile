##
# builder image
##
FROM rust:1-alpine AS builder

ARG REPO_DIR

RUN apk --no-cache add \
  musl-dev \
  protobuf-dev \
  g++ \
  clang15-dev \
  linux-headers \
  wasm-pack \
  openssl-dev \
  postgresql-dev

RUN rustup component add rustfmt

WORKDIR /work

ENV RUSTFLAGS="-C target-feature=-crt-static -C link-args=-s" \
  CARGO_REGISTRIES_CRATES_IO_PROTOCOL="sparse"

# Cache dependencies
RUN cargo init
COPY "$REPO_DIR"/Cargo.toml "$REPO_DIR"/Cargo.lock ./
RUN cargo build --release && \
    rm -r src/

COPY "$REPO_DIR" .
RUN cargo build --release


##
# runtime image
##
FROM alpine

RUN apk --no-cache add \
  libstdc++ \
  postgresql-client

USER 13337:13337

COPY --from=builder /work/target/release/kaspa-db-filler-ng /usr/local/bin/

ENTRYPOINT ["kaspa-db-filler-ng"]
CMD ["--help"]
