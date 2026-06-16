# Task Completion Checklist

Run in order after any coding change:

1. `cargo fmt` — format code
2. `cargo clippy -- -D warnings` — lint (fail on warnings)
3. `cargo test` — run test suite
4. `cargo build` — confirm clean compile

All four must pass before a task is considered done.
