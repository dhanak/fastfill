FROM rust:1.99.0-bookworm

WORKDIR /work
COPY Cargo.toml Cargo.lock build.rs README.md ./
COPY src/ src/
COPY tests/ tests/
COPY examples/ examples/
COPY assets/ assets/
COPY docs/samples/ docs/samples/
RUN cargo test --locked --no-default-features --no-run

CMD ["cargo", "test", "--locked", "--no-default-features"]
