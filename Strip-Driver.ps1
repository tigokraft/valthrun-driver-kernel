param(
    [Parameter(Mandatory)] [string] $InputFile,
    [Parameter(Mandatory)] [string] $OutputFile
)

if(-not (Test-Path -Path $InputFile)) {
    Write-Host "File '$InputFile' does not exists"
    exit 1
}

& '.\tools\CFF Explorer.exe' .\strip-driver.cff $InputFile $OutputFile | Out-Null
$log = $(Get-Content "strip_driver.log")
$success = 0
foreach($line in $log.Split("\n")) {
    if($line -eq "-- success --") {
        $success = 1
        continue
    }

    Write-Host $line
}
if(-not $success) {
    exit 1
} else {
    Write-Host "Driver successfully patched"
}