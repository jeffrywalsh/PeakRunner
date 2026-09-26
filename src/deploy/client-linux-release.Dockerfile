# Release recipe for Linux x86-64 clients: the standalone build and the
# launcher-managed (external-map) build, exported from an artifacts-only
# stage (`--target artifacts --output type=local,dest=...`). Same base and
# packages as client-linux.Dockerfile.
FROM rust:1-bookworm@sha256:ae1a730a949f727611a5c684e1e26e5a9bb9885b34f65a442744ca8a61c86ca5 AS build
RUN apt-get update && apt-get install -y --no-install-recommends libasound2-dev libudev-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libxi-dev libxcursor-dev libxrandr-dev ca-certificates
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY examples ./examples
COPY crates ./crates
COPY assets/maps ./assets/maps
ENV CARGO_BUILD_JOBS=4
RUN cargo build --locked --release -p peakrunner --bin peakrunner && cp target/release/peakrunner /peakrunner-standalone \
 && cargo build --locked --release -p peakrunner --bin peakrunner --features external-map && cp target/release/peakrunner /peakrunner-managed
FROM scratch AS artifacts
COPY --from=build /peakrunner-standalone /peakrunner-managed /
