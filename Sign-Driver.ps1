param([Parameter(Mandatory)] [string] $Target)
if (-not (Test-Path -Path $Target)) {
    Write-Host "File '$Target' does not exists"
    exit 1
}

$ErrorActionPreference = "SilentlyContinue"
$WDK = $(Get-ItemPropertyValue -Path 'HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots\' -Name 'KitsRoot10' -ErrorAction SilentlyContinue)
$ErrorActionPreference = "Continue"
if ([string]::IsNullOrEmpty($WDK)) {
    Write-Host "Missing WDK path"
    exit 1
}

$WDKBin = "$WDK\bin\10.0.22621.0\x64\"
if (-not (Test-Path -Path $WDKBin)) {
    Write-Host "Missing Windows kit for 10.0.22621.0"
    exit 1
}

if (-not (Test-Path "$PSScriptRoot\DriverCertificate.cer")) {
    Write-Host "Generating new certificate"
    & "$WDKBin\makecert.exe" -r -pe -ss PrivateCertStore -n CN=DriverCertificate $PSScriptRoot\DriverCertificate.cer
    if (-not $?) {
        Write-Host "Failed to generate certificate"
        exit 1
    }
}
else {
    Write-Host "Certificate already exists"
}

Write-Host "Signing"
& "$WDKBin\signtool.exe" sign /a /v /s PrivateCertStore /n DriverCertificate /t http://timestamp.digicert.com /fd SHA256 $Target
if (-not $?) {
    Write-Host "Failed to sign target"
    exit 1
}