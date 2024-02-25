Push-Location "driver"
cargo b -v -r
Pop-Location
if (-not $?) {
    Write-Host "Failed to build driver"
    exit 1
}

& "./Strip-Driver.ps1" -InputFile ./target/x86_64-pc-windows-msvc/release/driver.dll -OutputFile ./target/x86_64-pc-windows-msvc/release/valthrun-driver.sys
if (-not $?) {
    Write-Host "Failed to strip driver"
    exit 1
}

& "./Sign-Driver.ps1" ./target/x86_64-pc-windows-msvc/release/valthrun-driver.sys
if (-not $?) {
    exit 1
}