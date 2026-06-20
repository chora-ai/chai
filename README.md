# chai

A multi-agent management system.

## Documentation

For user guides and other documentation, see [docs](docs/README.md).

## Packages

- **`crates/cli`** — Command-line interface for the multi-agent management system
- **`crates/desktop`** — Graphical user interface for the multi-agent management system
- **`crates/lib`** — Shared business logic for the multi-agent management system

## Getting Started

### Shell

```bash
# Nix shell
nix develop
```

### Test

```bash
# Test all crates
cargo test

# Test a specific crate
cargo test -p cli
cargo test -p desktop
cargo test -p lib
```

### Build

```bash
# Build all crates
cargo build

# Build CLI
cargo build -p cli

# Build desktop
cargo build -p desktop

# Build with experimental adapters
cargo build -p cli --features matrix,signal
cargo build -p desktop --features matrix,signal
```

### Install

```bash
# Install CLI
cargo install --path crates/cli

# Install desktop
cargo install --path crates/desktop

# Install with experimental adapters
cargo install --path crates/cli --features matrix,signal
cargo install --path crates/desktop --features matrix,signal
```

### Run

```bash
# Run CLI
chai help

# Run Desktop
chai-desktop
```

## License

Licensed under the GPL-3.0: https://www.gnu.org/licenses/gpl-3.0.html
