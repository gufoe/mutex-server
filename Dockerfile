FROM rust:bullseye as builder
RUN rustup target add x86_64-unknown-linux-musl

RUN mkdir -p /app/src && echo 'fn main(){}' > /app/src/main.rs
WORKDIR /app
COPY Cargo.* ./
RUN cargo build --release --target x86_64-unknown-linux-musl
COPY src src
RUN touch src/main.rs && cargo build --release --target x86_64-unknown-linux-musl
RUN cargo install --target x86_64-unknown-linux-musl --path .
RUN strip /usr/local/cargo/bin/mutex-server


FROM bash
ADD https://github.com/Yelp/dumb-init/releases/download/v1.2.5/dumb-init_1.2.5_x86_64 /usr/local/bin/dumb-init
RUN chmod +x /usr/local/bin/dumb-init
COPY --from=builder /usr/local/cargo/bin/mutex-server /mutex-server
ENTRYPOINT ["dumb-init", "/mutex-server", "--bind", "0.0.0.0:9922"]
