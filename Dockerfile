# Reproducible, host-toolchain-free build of the Borderlands 2 save tool.
# (Currently builds the round-trip proof-of-concept; will grow with the project.)
#
#   docker build -t bl2edit .
#   docker run --rm bl2edit                 # runs the PoC on the bundled sample
#   docker run --rm -v "$PWD/samples":/app/samples bl2edit samples/save0002.sav

# ---- Stage 1: compile with the official Rust image ----
FROM rust:1 AS builder
WORKDIR /src
COPY poc-roundtrip/ ./poc-roundtrip/
WORKDIR /src/poc-roundtrip
RUN cargo build --release

# ---- Stage 2: minimal runtime image with just the binary ----
FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /src/poc-roundtrip/target/release/poc-roundtrip /usr/local/bin/poc-roundtrip
COPY samples/ ./samples/
ENTRYPOINT ["poc-roundtrip"]
CMD ["samples/save0001.sav"]
