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
COPY "$REPO_DIR"/kaspad/Cargo.toml ./kaspad/
COPY "$REPO_DIR"/database/Cargo.toml ./database/
COPY "$REPO_DIR"/mapping/Cargo.toml ./mapping/
COPY "$REPO_DIR"/indexer/Cargo.toml ./indexer/
RUN mkdir kaspad/src database/src mapping/src indexer/src && \
    echo "pub fn placeholder() {}" > kaspad/src/lib.rs && \
    echo "pub fn placeholder() {}" > database/src/lib.rs && \
    echo "pub fn placeholder() {}" > mapping/src/lib.rs && \
    echo "pub fn placeholder() {}" > indexer/src/lib.rs && \
    echo "fn main() {}" > indexer/src/main.rs && \
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

COPY LICENSE /usr/local/bin/
COPY --from=builder /work/target/release/simply-kaspa-indexer /usr/local/bin/
RUN ln -s /usr/local/bin/simply-kaspa-indexer /usr/local/bin/kaspa-db-filler-ng

USER 13337:13337

ENTRYPOINT ["simply-kaspa-indexer"]
CMD ["--help"]
