# Builds release binaries for Radicle.
ARG RUST_VERSION="1.85"
ARG ALPINE_VERSION="3.20"

FROM rust:${RUST_VERSION}-alpine${ALPINE_VERSION} as builder
LABEL maintainer="Erik Kundt <erik@zirkular.io>"
WORKDIR /src
COPY . .

ARG TZ
ARG LC_ALL
ARG SOURCE_DATE_EPOCH
ARG PACKAGE_NAME
ARG BINARY_NAME
ARG VERSION
ARG GIT_HEAD
ARG SDK_PATH
ARG CONFIG_PATH

# Copy cargo configuration we're going to use to specify compiler options.
RUN mkdir -p .cargo && cp "$CONFIG_PATH" .cargo/config.toml
# Install dependencies.
RUN apk update && apk add --no-cache git musl-dev xz zig
# Add cargo targets.
RUN rustup target add \
    x86_64-unknown-linux-musl \
    aarch64-unknown-linux-musl \
    x86_64-apple-darwin \
    aarch64-apple-darwin

# Install dependencies for cross-compiling to macOS.
# We use Zig as the linker to perform the compilation from a Linux host.
# Zig is not yet available on Debian, so we download the official binary.
# Compilation is done via `cargo-zigbuild` which is a wrapper around `zig`.
RUN cargo install --locked cargo-zigbuild@0.20.0


# Parts of the macOS SDK are required to build Radicle, we make these available
# here. So far only `CoreFoundation` and `Security` frameworks are needed.
RUN xz -d -c "$SDK_PATH" | tar -x
# This env var is used by `cargo-zigbuild` to find the SDK.
ENV SDKROOT /src/macos-sdk-11.3

# Build binaries.
RUN cargo zigbuild --locked --release \
    --target=x86_64-apple-darwin \
    --target=aarch64-apple-darwin \
    --target=aarch64-unknown-linux-musl \
    --target=x86_64-unknown-linux-musl \
    -p "$PACKAGE_NAME"

# Now copy the files to a new image without all the intermediary artifacts to
# save some space.
FROM alpine:3.20 as packager

ARG VERSION
ARG SOURCE_DATE_EPOCH
ARG PACKAGE_NAME
ARG BINARY_NAME

COPY --from=builder \
    /src/target/x86_64-unknown-linux-musl/release/${BINARY_NAME} \
    /builds/x86_64-unknown-linux-musl/bin/
COPY --from=builder \
    /src/target/aarch64-unknown-linux-musl/release/${BINARY_NAME} \
    /builds/aarch64-unknown-linux-musl/bin/
COPY --from=builder \
    /src/target/aarch64-apple-darwin/release/${BINARY_NAME} \
    /builds/aarch64-apple-darwin/bin/
COPY --from=builder \
    /src/target/x86_64-apple-darwin/release/${BINARY_NAME} \
    /builds/x86_64-apple-darwin/bin/

# Create and compress reproducible archive.
WORKDIR /builds
RUN apk update && apk add --no-cache tar xz
RUN find * -maxdepth 0 -type d -exec mv '{}' "$PACKAGE_NAME-$VERSION-{}" \; && \
    find * -maxdepth 0 -type d -exec tar \
    --sort=name \
    --verbose \
    --mtime="@$SOURCE_DATE_EPOCH" \
    --owner=0 \
    --group=0 \
    --numeric-owner \
    --format=posix \
    --pax-option=exthdr.name=%d/PaxHeaders/%f,delete=atime,delete=ctime \
    --mode='go+u,go-w' \
    --remove-files \
    --create --xz \
    --file="{}.tar.xz" \
    '{}' \;
