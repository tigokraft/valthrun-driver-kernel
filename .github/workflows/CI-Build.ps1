cargo b -v -r
if(-not $?) {
    Write-Host "Failed to build driver"
    exit 1
}

cp ./target/x86_64-pc-windows-msvc/release/driver.dll ./target/x86_64-pc-windows-msvc/release/valthrun-driver.sys
if(-not $?) {
    Write-Host "Failed to copy driver"
    exit 1
}

& "./Sign-Driver.ps1" ./target/x86_64-pc-windows-msvc/release/valthrun-driver.sys
if(-not $?) {
    exit 1
}