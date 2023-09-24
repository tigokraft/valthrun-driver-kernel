$cleanName = "valthrun-driver"
$mode = "debug"

Write-Host "Stopping & deleting driver"
sc.exe stop $cleanName
sc.exe delete $cleanName

$path = "$pwd\target\x86_64-pc-windows-msvc\$mode\$cleanName.sys"
Write-Host "Installing & starting driver ($path)"
sc.exe create $cleanName type= kernel start= demand error= normal binPath= $path DisplayName= $cleanName
sc.exe start $cleanName
