# Conventions

Project is scaffold-only; no codebase conventions established yet.

Defaults to apply until explicit conventions emerge:
- Standard Rust naming: `snake_case` for functions/variables/modules, `PascalCase` for types/traits, `SCREAMING_SNAKE_CASE` for constants.
- Format with `cargo fmt` (default rustfmt settings; no `rustfmt.toml` present).
- Lint with `cargo clippy` (no `.clippy.toml` present).
- No comment style conventions established; follow Rust doc-comment norms (`///` for public items).
