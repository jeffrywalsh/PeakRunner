FROM rust:1-bookworm@sha256:ae1a730a949f727611a5c684e1e26e5a9bb9885b34f65a442744ca8a61c86ca5
RUN apt-get update && apt-get install -y --no-install-recommends libasound2-dev libudev-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libxi-dev libxcursor-dev libxrandr-dev libvulkan1 mesa-vulkan-drivers xvfb xauth ca-certificates
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY examples ./examples
COPY crates ./crates
COPY assets/maps/raindance ./assets/maps/raindance
COPY assets/maps/tower-complex ./assets/maps/tower-complex
COPY assets/maps/cairnhold ./assets/maps/cairnhold
COPY assets/maps/frostline ./assets/maps/frostline
COPY assets/maps/dustreach ./assets/maps/dustreach
COPY assets/maps/longfield ./assets/maps/longfield
COPY assets/maps/highgoal ./assets/maps/highgoal
COPY assets/maps/ozarktic-blast ./assets/maps/ozarktic-blast
COPY assets/maps/reefbreak ./assets/maps/reefbreak
ENV CARGO_BUILD_JOBS=4
RUN cargo build --locked --release -p peakrunner --bin peakrunner --example launch_smoke && cargo build --locked --release -p peakrunner-net --example public_smoke
