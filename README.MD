# Valthrun Kernel Driver
Thanks Matthias for the buitifull writeup on [how to get started writing a kernel driver with Rust](https://not-matthias.github.io/posts/kernel-driver-with-rust/).

## Build
### Requirements
- Visual Studio 2022 (located at `%ProgramFiles%\Microsoft Visual Studio\2022\Community`)
- Windows Kernel Development Kit
- Cargo Make (`cargo install --force cargo-make`)

```ps1
# Switch to the kernel driver directory
cd kernel/driver

# Create a release build.
# The result will be located at "../target/x86_64-pc-windows-msvc/release/valthrun-driver.sys"
cargo make --profile production sign
```


# Stelth TODO
- https://www.unknowncheats.me/forum/anti-cheat-bypass/444289-read-process-physical-memory-attach.html
- `KeUserModeCallback`
https://www.unknowncheats.me/forum/anti-cheat-bypass/347320-communication-via-callbacks.html