FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release --bin peakrunner-directory --bin peakrunner-server

FROM debian:bookworm-slim
COPY --from=build /src/target/release/peakrunner-directory /usr/local/bin/peakrunner-directory
COPY --from=build /src/target/release/peakrunner-server /usr/local/bin/peakrunner-server
EXPOSE 7780 7781
USER nobody
CMD ["peakrunner-directory", "0.0.0.0:7780"]
