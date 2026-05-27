# HAL for Rust

A Rust implementation of the Hybrid Automation Language (HAL).

This repository provides a high-performance, memory-safe Rust library (`hal`) for embedding the HAL interpreter into any Rust application.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
hal = { git = "https://github.com/Igazine/hal-rust.git" }
```

## Features

- **High Performance**: Optimized tree-walking interpreter.
- **AST Caching**: Eliminates parsing overhead for repeated execution.
- **Embedded Friendly**: Minimal resource footprint (tested on ARM Linux).
- **Standard Library**: Full parity with HAL 1.0 specifications.

## Example Runner

An example CLI runner is included in `examples/runner`. Note that the runner requires the universal conformance suite located in the `hal` submodule.

To fetch submodules after cloning:

```bash
git submodule update --init --recursive
```

To run the conformance tests:

```bash
cargo run --example runner
```

## Project Links

- **HAL Core Repo**: [Igazine/hal](https://github.com/Igazine/hal)
- **Official Documentation**: [https://igazine.github.io/hal/](https://igazine.github.io/hal/)

## License

This project is licensed under the MIT License.
