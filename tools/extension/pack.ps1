# Packages the English Chromium extension for Microsoft Edge (Load unpacked).
# Keeps the existing manifest key so installed extensions retain their native-host ID.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$src = Join-Path $root "extension"
$out = Join-Path $root "target\extension"
$cargo = Get-Content (Join-Path $root "Cargo.toml") -Raw
$version = [regex]::Match($cargo, '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
if ($version -notmatch '^\d+(\.\d+){0,3}$') { throw "version ${version}: browsers take only numbers like 1.2.3" }

# Remove obsolete Firefox, Safari and store packages from earlier builds too.
if (Test-Path $out) { Remove-Item -LiteralPath $out -Recurse -Force }
$dir = Join-Path $out "chromium"
New-Item -ItemType Directory -Force $dir | Out-Null
foreach ($f in "background.js", "video.js", "page.js", "popup.html", "popup.css", "popup.js", "popup-ui.js", "popup-downloads.js") {
  Copy-Item (Join-Path $src $f) $dir
}
Copy-Item (Join-Path $src "icons") (Join-Path $dir "icons") -Recurse
New-Item -ItemType Directory -Force (Join-Path $dir "_locales") | Out-Null
Copy-Item (Join-Path $src "_locales\en") (Join-Path $dir "_locales\en") -Recurse
$json = Get-Content (Join-Path $src "manifest.json") -Raw
$json = ([regex]'("version"\s*:\s*)"[^"]*"').Replace($json, "`${1}`"$version`"", 1)
[IO.File]::WriteAllText((Join-Path $dir "manifest.json"), $json, (New-Object Text.UTF8Encoding $false))
Copy-Item (Join-Path $root "LICENSE") (Join-Path $dir "LICENSE.txt")
Copy-Item (Join-Path $root "ATTRIBUTION.md") (Join-Path $dir "ATTRIBUTION.txt")

Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
$zip = Join-Path $out "mdm-chromium.zip"
$archive = [System.IO.Compression.ZipFile]::Open($zip, [System.IO.Compression.ZipArchiveMode]::Create)
try {
  foreach ($f in Get-ChildItem -LiteralPath $dir -Recurse -File | Sort-Object FullName) {
    $name = $f.FullName.Substring($dir.Length).TrimStart('\', '/').Replace('\', '/')
    [void][System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $f.FullName, $name)
  }
} finally { $archive.Dispose() }
"built $out (English Microsoft Edge extension, mdm-chromium.zip; version $version)"
