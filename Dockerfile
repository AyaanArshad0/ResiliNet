FROM rust:latest as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/ghost_tunnel /usr/local/bin/
# Install runtime networking tools
RUN apt-get update && apt-get install -y iproute2 iptables
ENTRYPOINT ["ghost_tunnel"]
