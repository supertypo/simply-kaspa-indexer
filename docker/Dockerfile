##
# builder image
##
FROM rust:1.81-alpine AS builder

ARG REPO_DIR=.

RUN apk --no-cache add \
  musl-dev \
  protobuf-dev \
  g++ \
  clang15-dev \
  linux-headers \
  wasm-pack \
  openssl-dev \
  postgresql-dev

WORKDIR /work

ENV RUSTFLAGS="-C target-feature=-crt-static -C link-args=-s" \
  CARGO_REGISTRIES_CRATES_IO_PROTOCOL="sparse"

# Cache dependencies
COPY "$REPO_DIR"/Cargo.toml "$REPO_DIR"/Cargo.lock ./
COPY "$REPO_DIR"/database/Cargo.toml ./database/
COPY "$REPO_DIR"/filler/Cargo.toml ./filler/
RUN mkdir database/src filler/src && \
    echo "pub fn placeholder() {}" > database/src/lib.rs && \
    echo "pub fn placeholder() {}" > filler/src/lib.rs && \
    echo "fn main() {}" > filler/src/main.rs && \
    cargo build --release

COPY "$REPO_DIR" .
RUN find . -type f \( -name lib.rs -or -name main.rs \) -exec touch {} +
RUN cargo build --release


##
# runtime image
##
FROM alpine

RUN apk --no-cache add \
  libstdc++ \
  postgresql-client

USER 13337:13337

COPY LICENSE /usr/local/bin/
COPY --from=builder /work/target/release/kaspa-db-filler-ng /usr/local/bin/

ENTRYPOINT ["kaspa-db-filler-ng"]
CMD ["--help"]
