FROM rust:1-bookworm@sha256:ae1a730a949f727611a5c684e1e26e5a9bb9885b34f65a442744ca8a61c86ca5 AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY assets/maps/raindance ./assets/maps/raindance
COPY assets/maps/tower-complex ./assets/maps/tower-complex
COPY assets/maps/cairnhold ./assets/maps/cairnhold
COPY assets/maps/frostline ./assets/maps/frostline
COPY assets/maps/dustreach ./assets/maps/dustreach
COPY assets/maps/longfield ./assets/maps/longfield
COPY assets/maps/highgoal ./assets/maps/highgoal
COPY assets/maps/ozarktic-blast ./assets/maps/ozarktic-blast
COPY assets/maps/reefbreak ./assets/maps/reefbreak
COPY crates ./crates
RUN cargo build --locked --release -p peakrunner-server --bin peakrunner-server

FROM debian:bookworm-slim@sha256:88200866dfff7ea7f5cbcb6ec7c8a701889efe6fe859fe64d6990e4b07ea4171
COPY --from=build /src/target/release/peakrunner-server /usr/local/bin/peakrunner-server
COPY --from=build /src/assets/maps/raindance /opt/peakrunner/maps/raindance
ENV PEAKRUNNER_MAP_PACK=/opt/peakrunner/maps/raindance
EXPOSE 7777/udp
USER nobody
CMD ["peakrunner-server"]
