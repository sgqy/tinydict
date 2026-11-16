param([switch]$SkipDownload)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    if (-not $SkipDownload) {
        python scripts/fetch_sources.py
        if ($LASTEXITCODE -ne 0) { throw 'Source download failed.' }
        python scripts/fetch_wiktionary.py simple-wiktionary wiktionary
        if ($LASTEXITCODE -ne 0) { throw 'Wiktionary download failed.' }
    }
    cargo build --release --workspace --locked
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Cannot locate Cargo output directory.' }
    $releaseDir = Join-Path $metadata.target_directory 'release'
    $importer = Join-Path $releaseDir 'tinydict-import.exe'
    $packer = Join-Path $releaseDir 'tinydict-pack.exe'
    $sourceNames = @('cloudbytes', 'wordset', 'simple-wiktionary', 'wiktionary')
    foreach ($sourceName in $sourceNames) {
        $sourceDir = Join-Path $projectRoot "data/sources/$sourceName"
        $output = Join-Path $projectRoot "data/dictionaries/$sourceName.tdict.zst"
        $legacy = Join-Path $projectRoot "data/dictionaries/$sourceName.tdict"
        if (-not (Test-Path -LiteralPath $output)) {
            if (Test-Path -LiteralPath $legacy) {
                & $packer --input $legacy --output $output --verify
                if ($LASTEXITCODE -ne 0) { throw "Repack failed: $sourceName" }
                continue
            }
            $pin = Get-Content -Raw (Join-Path $sourceDir 'source.json') | ConvertFrom-Json
            if ($sourceName -eq 'cloudbytes') {
                $inputPath = Join-Path $sourceDir 'dictionary.jsonl.gz'
                $licensePath = Join-Path $sourceDir 'LICENSE-DATA.md'
            } elseif ($sourceName -eq 'wordset') {
                $inputPath = Join-Path $sourceDir 'data'
                $licensePath = Join-Path $sourceDir 'LICENSE'
            } else {
                $inputPath = Join-Path $sourceDir 'dictionary.jsonl.gz'
                $licensePath = Join-Path $sourceDir 'LICENSE.txt'
            }
            & $importer $sourceName --input $inputPath --license $licensePath --revision $pin.revision --output $output
            if ($LASTEXITCODE -ne 0) { throw "Import failed: $sourceName" }
        } else {
            Write-Host "Keeping existing package: $output"
        }
    }
    $destination = Join-Path $projectRoot 'dist'
    $dictionaryDestination = Join-Path $destination 'data/dictionaries'
    New-Item -ItemType Directory -Path $dictionaryDestination -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $releaseDir 'tinydict.exe') -Destination $destination
    Copy-Item -LiteralPath $importer -Destination $destination
    Copy-Item -LiteralPath $packer -Destination $destination
    Copy-Item -LiteralPath (Join-Path $releaseDir 'tinydict-inspect.exe') -Destination $destination
    foreach ($sourceName in $sourceNames) {
        Copy-Item -LiteralPath (Join-Path $projectRoot "data/dictionaries/$sourceName.tdict.zst") -Destination $dictionaryDestination
    }
    Write-Host "Ready: $(Join-Path $destination 'tinydict.exe')"
} finally {
    Pop-Location
}
