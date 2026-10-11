$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path $PSScriptRoot -Parent
$taskTools = Join-Path $taskRoot '.tools'
$taskLlvm = Get-ChildItem -LiteralPath $taskTools -Directory -Filter 'llvm-mingw-*' -ErrorAction SilentlyContinue | Select-Object -First 1
$taskSdk = Join-Path $taskTools 'msvc'
$taskDuck = Join-Path $taskTools 'duckdb'
if (!(Test-Path -LiteralPath (Join-Path $taskDuck 'duckdb.dll'))) { & (Join-Path $PSScriptRoot 'bootstrap-duckdb.ps1') }
$env:DUCKDB_LIB_DIR = $taskDuck
$env:DUCKDB_INCLUDE_DIR = $taskDuck
$env:PATH = "$taskDuck;$env:PATH"
if ($taskLlvm -and (Test-Path -LiteralPath $taskSdk)) {
    $taskBin = Join-Path $taskLlvm.FullName 'bin'
    foreach ($taskAlias in @('clang-cl.exe','lld-link.exe')) {
        $taskOriginal = if ($taskAlias -eq 'clang-cl.exe') { (Get-ChildItem -LiteralPath $taskBin -Filter 'clang-*.exe' | Where-Object Name -Match '^clang-\d+\.exe$' | Select-Object -First 1).Name } else { 'ld.lld.exe' }
        Copy-Item -LiteralPath (Join-Path $taskBin $taskOriginal) -Destination (Join-Path $taskBin $taskAlias) -Force
    }
    $env:PATH = "$taskBin;$env:PATH"
    $env:CC_x86_64_pc_windows_msvc = 'clang-cl.exe'
    $env:CXX_x86_64_pc_windows_msvc = 'clang-cl.exe'
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = 'lld-link.exe'
    $env:CXXFLAGS_x86_64_pc_windows_msvc = '/DDUCKDB_STATIC_BUILD'
    $env:INCLUDE = (@('crt/include','sdk/include/ucrt','sdk/include/shared','sdk/include/um','sdk/include/winrt') | ForEach-Object { Join-Path $taskSdk $_ }) -join ';'
    $env:LIB = (@('crt/lib/x86_64','sdk/lib/ucrt/x86_64','sdk/lib/um/x86_64') | ForEach-Object { Join-Path $taskSdk $_ }) -join ';'
    $taskRcWrapper = Join-Path $taskTools 'rc-wrapper.exe'
    if (!(Test-Path -LiteralPath $taskRcWrapper)) {
        & rustc (Join-Path $PSScriptRoot 'rc-wrapper.rs') -C linker=lld-link.exe -o $taskRcWrapper
        if ($LASTEXITCODE -ne 0) { throw 'Could not build UTF-8 resource compiler wrapper' }
    }
    $env:WORKBENCH_LLVM_RC = Join-Path $taskBin 'llvm-rc.exe'
    $env:RC = $taskRcWrapper
}
