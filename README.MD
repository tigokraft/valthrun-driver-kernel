# Valthrun Kernel Driver

This is the implementation for the Valthrun driver protocol as a kernel driver.

## Build

### Requirements

- Visual Studio 2022
- Windows Kernel Development Kit
- Cargo Make (`cargo install --force cargo-make`)
- LLVM (will throw an error building without it)

### Build the driver

```ps1
# Go to ether the standalone or UEFI driver
cd driver-standalone

# Create a release build.
# The result will be located at "../target/x86_64-pc-windows-msvc/release/valthrun-driver.sys"
cargo b -r

```
