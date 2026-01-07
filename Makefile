build:
	cargo build --release

run-server:
	sudo ./target/release/ghost_tunnel --server --bind 0.0.0.0:8000 --tun-ip 10.0.0.1

run-client:
	sudo ./target/release/ghost_tunnel --peer 127.0.0.1:8000 --tun-ip 10.0.0.2

test-chaos:
	sudo ./scripts/simulate_loss.sh
