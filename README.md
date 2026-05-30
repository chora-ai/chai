# chai

A multi-agent management system.

## Documentation

For user guides and other user documentation, see [documentation](docs/README.md).

## Packages

- **`crates/cli`** — A command-line interface for the multi-agent management system
- **`crates/desktop`** — A graphical user-interface for the multi-agent management system
- **`crates/lib`** — All shared business logic for the multi-agent management system

## Commands

```bash
# Test everything
cargo test

# Build everything
cargo build

# Build specific crates
cargo build -p cli
cargo build -p desktop
cargo build -p lib

# Run the command-line interface
cargo run -p cli -- help

# Run the desktop application
cargo run -p desktop
```

Use `--features matrix` to build or run with the `matrix` adaptor.

## Command-Line Interface

Install the CLI locally:

```bash
cargo install --path crates/cli
```

Use `--features matrix` to install the `matrix` adaptor.

Run the installed CLI:

```bash
chai help
```

## Desktop Application

Install the app locally:

```bash
cargo install --path crates/desktop
```

Use `--features matrix` to install the `matrix` adaptor.

Run the installed app:

```bash
chai-desktop
```
