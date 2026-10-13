param(
    [ValidateSet('run', 'check', 'build')]
    [string]$Action = 'run'
)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
$cargoPath = if ($cargoCommand) { $cargoCommand.Source } else { Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
if (-not (Test-Path -LiteralPath $cargoPath)) {
    throw 'Rust is missing. Install stable Rust for x86_64-pc-windows-msvc using rustup.'
}
# Cargo invokes rustc and linker helpers through PATH.
$env:PATH = "$(Split-Path $cargoPath -Parent);$env:PATH"
function Invoke-CargoChecked {
    param([string[]]$CargoArguments)
    & $cargoPath @CargoArguments
    if ($LASTEXITCODE -ne 0) { throw "Cargo failed with exit code $LASTEXITCODE" }
}
switch ($Action) {
    'run' { Invoke-CargoChecked @('run', '-p', 'bl4-desktop', '--locked') }
    'build' { Invoke-CargoChecked @('build', '-p', 'bl4-desktop', '--release', '--locked') }
    'check' {
        Invoke-CargoChecked @('fmt', '--all', '--', '--check')
        Invoke-CargoChecked @('test', '--workspace', '--locked')
        Invoke-CargoChecked @('clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings')
    }
}
