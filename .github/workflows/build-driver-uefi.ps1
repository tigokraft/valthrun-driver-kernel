Push-Location "driver-uefi"
cargo b -v -r
Pop-Location
if (-not $?) {
    Write-Host "Failed to build driver"
    exit 1
}

& "./Strip-Driver.ps1" -InputFile ./target/x86_64-pc-windows-msvc/release/driver_uefi.dll -OutputFile ./target/x86_64-pc-windows-msvc/release/driver_uefi.dll
if (-not $?) {
    Write-Host "Failed to strip driver"
    exit 1
}