# Default lnd server base URL used by client recipes.
server_url := "http://127.0.0.1:8765"
# Default shared bearer token used by local development recipes.
bearer_token := "dev-token"
# Default logical discovery domain used by announce, discover, and watch recipes.
network_id := "office-a"
# Default service name used by SDK and CLI examples.
service := "_demo._tcp"
# Default service port announced by local recipes.
port := "8080"
# Default display name announced by local recipes.
display_name := "devbox-a"
# Default server config file passed to `just server`.
config_file := "config.toml.example"

alias b := build
alias br := build-release
alias l := clippy
alias t := test

# Show available recipes.
default:
    @just --list

# Format Rust sources.
fmt:
    cargo fmt --all

# Lint the whole Rust workspace.
clippy:
    cargo clippy --workspace --all-targets

# Run Rust tests.
test:
    cargo test --workspace

# Run CLI focused tests.
test-cli:
    cargo test --test cli

# Run integration tests.
test-integration:
    cargo test --test integration

# Run the FFI smoke test.
test-ffi:
    cargo test --test ffi_smoke

# Build debug artifacts.
build:
    cargo build

# Build release artifacts.
build-release:
    cargo build --release

# Remove Rust build artifacts.
clean:
    cargo clean

# Run lnd-server with the example config file.
server:
    cargo run --bin lnd-server -- --config {{ config_file }}

# Run lnd-server with custom CLI args.
server-raw *args:
    cargo run --bin lnd-server -- {{ args }}

# Run lnd-client with default server settings.
client *args:
    cargo run --bin lnd-client -- --server-url {{ server_url }} --bearer-token {{ bearer_token }} {{ args }}

# Announce one service with default local settings.
announce *args:
    cargo run --bin lnd-client -- --server-url {{ server_url }} --bearer-token {{ bearer_token }} announce --network-id {{ network_id }} --service {{ service }} --port {{ port }} --display-name {{ display_name }} {{ args }}

# Discover peers with default server settings.
discover *args:
    cargo run --bin lnd-client -- --server-url {{ server_url }} --bearer-token {{ bearer_token }} discover --network-id {{ network_id }} {{ args }}

# Watch peer events with default server settings.
watch *args:
    cargo run --bin lnd-client -- --server-url {{ server_url }} --bearer-token {{ bearer_token }} watch --network-id {{ network_id }} {{ args }}

# Run one Rust example by name.
example-rust name:
    cargo run --example {{ name }}

# Run the Go SDK example.
example-go:
    tmpdir="$(mktemp -d -t lnd-go-example.XXXXXX)" && \
    trap 'rm -rf "$tmpdir"' EXIT && \
    cp examples/sdk/go/main.go "$tmpdir/main.go" && \
    printf '%s\n' \
      'module lnd-go-example' \
      '' \
      'go 1.23' \
      '' \
      'require github.com/azazo1/lnd/bindings/go v0.0.0' \
      '' \
      'replace github.com/azazo1/lnd/bindings/go => {{ justfile_directory() }}/bindings/go' \
      > "$tmpdir/go.mod" && \
    cd "$tmpdir" && go run .

# Install Node.js SDK dependencies with bun.
node-install:
    cd bindings/node && bun install

# Run the Node.js SDK example.
example-node: build-release node-install
    bun run ./examples/sdk/node/discover.js

# Build the Python wheel.
python-wheel:
    cd bindings/python && maturin build --release

# Run the Python SDK example against the newest built wheel.
example-python: python-wheel
    uv run --with "$(ls -1t target/wheels/lnd_sdk-*.whl | head -n 1)" python ./examples/sdk/python/discover.py

# Run Go SDK tests.
go-test:
    cd bindings/go && go test ./...

# Build release artifacts and the Python wheel.
dist: build-release python-wheel
