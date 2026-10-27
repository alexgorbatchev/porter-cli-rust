run *args:
    cargo run --bin porter -- {{args}}

run-ai *args:
    AGENT=1 cargo run --bin porter -- {{args}}

build:
    cargo build --release --bin porter
    mkdir -p bin
    cp target/release/porter bin/porter

test:
    cargo test --all-targets

lint:
    rustfmt --check --edition 2024 --config skip_children=true src/lib.rs src/main.rs src/process.rs src/socket_diag.rs src/ui.rs tests/behavior.rs
    cargo clippy --all-targets -- -D warnings

coverage:
    cargo llvm-cov --all-targets --fail-under-lines 90

check: lint test coverage
