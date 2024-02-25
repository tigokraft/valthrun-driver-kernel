function Build-BasicDriver() {
    $process = Start-Process -PassThru -Wait -NoNewWindow -WorkingDirectory "driver" -FilePath "cargo" -ArgumentList "b", "-v", "-r"
    if ($process.ExitCode -ne 0) {
        Write-Host "Failed to build driver ($($process.ExitCode))"
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
}

function Build-UefiDriver() {
    $process = Start-Process -PassThru -Wait -NoNewWindow -WorkingDirectory "driver-uefi" -FilePath "cargo" -ArgumentList "b", "-v", "-r"
    if ($process.ExitCode -ne 0) {
        Write-Host "Failed to build driver ($($process.ExitCode))"
        exit 1
    }

    & "./Strip-Driver.ps1" -InputFile ./target/x86_64-pc-windows-msvc/release/driver_uefi.dll -OutputFile ./target/x86_64-pc-windows-msvc/release/driver_uefi.dll
    if (-not $?) {
        Write-Host "Failed to strip driver"
        exit 1
    }
}

Build-BasicDriver
Build-UefiDriver