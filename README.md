# Astrolink

A standalone `no_std` Rust module for Canopus, generated from the official Rust
template. The initial scaffold uses Supervisor services and has no application
logic yet.

## Layout

```text
rust/
├── Canopus-Private/       # SDK and CLI checkout
└── astrolink/
    ├── Canopus.toml       # Module metadata and Canopus build configuration
    ├── Cargo.toml        # Independent Rust workspace and SDK dependencies
    └── src/
        └── lib.rs        # Module descriptor and lifecycle callbacks
```

SDK dependencies use paths relative to this project. Keep both directories next
to each other, or update the dependency paths and `CANOPUS_ROOT` when moving them.

## Build and package

Prerequisites: Rust with the `thumbv8m.main-none-eabi` target, Clang, LLD, and
`llvm-ar`. On macOS, install LLVM and LLD with `brew install llvm lld`.

Run from this directory:

```sh
rustup target add thumbv8m.main-none-eabi
export CANOPUS_ROOT="$(cd ../Canopus-Private && pwd)"

# Build the local Canopus CLI if needed.
cargo build --manifest-path "$CANOPUS_ROOT/Cargo.toml" -p canopus-cli

# On macOS, make Homebrew LLVM and LLD available.
# export PATH="$(brew --prefix llvm)/bin:$(brew --prefix lld)/bin:$PATH"

"$CANOPUS_ROOT/target/debug/canopus" build
"$CANOPUS_ROOT/target/debug/canopus" package
"$CANOPUS_ROOT/target/debug/canopus" package verify dist/astrolink-0.1.0.canopus
```

`canopus build` configures the target identity, device feature, Cortex-M33 flags,
abort-on-panic behavior, C constructor shim, linking, and ELF verification. Use it
for device builds. Build outputs go to `build/`; installation packages go to
`dist/`. Keep `Cargo.lock` in version control for reproducible dependency resolution.

The initial module is `portable`: one ELF can run across supported Band 11
firmware versions. Band 10 Pro requires a target-bound configuration with exact
firmware IDs in `Canopus.toml`.

## Development

`src/lib.rs` exposes the module descriptor and these callbacks:

- `canopus_mod_prepare`: initialize module state.
- `activate`: start module work; currently writes `astrolink active` to the log.
- `deactivate`: stop active work before teardown.
- `canopus_mod_stop`: release resources before unloading.
- `query`: publish status; currently writes the template status marker `0x4D4F4455`.

Add business logic as Rust modules under `src/` and call it from these callbacks.
Keep module ID and version consistent between `Canopus.toml`, `Cargo.toml`, and
the descriptor in `src/lib.rs`.

Check Rust formatting with:

```sh
cargo fmt --check
```
