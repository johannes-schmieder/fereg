$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$Build = Join-Path $Root "tests\mock_spi\build\windows"
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Build
New-Item -ItemType Directory -Force -Path $Build | Out-Null
Set-Location $Build

$Common = @(
    "/nologo", "/std:c11", "/O2", "/MT", "/W4", "/WX", "/TC",
    "/D_CRT_SECURE_NO_WARNINGS", "/DSYSTEM=STWIN32", "/I$Root\plugin"
)
$LoaderCommon = @(
    "/nologo", "/std:c11", "/O2", "/MT", "/W4", "/WX", "/TC",
    "/D_CRT_SECURE_NO_WARNINGS", "/DSYSTEM=STWIN32", "/I$Root\plugin"
)

$BridgeExe = Join-Path $Build "mock_spi_test.exe"
& cl.exe @Common "$Root\plugin\stplugin.c" "$Root\plugin\stata_bridge.c" `
    "$Root\tests\mock_spi\mock_spi.c" "/Fe:$BridgeExe"
if ($LASTEXITCODE -ne 0) { throw "Windows mock SPI bridge compilation failed" }
& $BridgeExe
if ($LASTEXITCODE -ne 0) { throw "Windows mock SPI bridge test failed" }

$StpluginObj = Join-Path $Build "stplugin.obj"
$BridgeObj = Join-Path $Build "stata_bridge.obj"
$DispatchObj = Join-Path $Build "dummy_dispatch.obj"
& cl.exe @Common /c "$Root\plugin\stplugin.c" "/Fo$StpluginObj"
if ($LASTEXITCODE -ne 0) { throw "stplugin mock object compilation failed" }
& cl.exe @Common /c "$Root\plugin\stata_bridge.c" "/Fo$BridgeObj"
if ($LASTEXITCODE -ne 0) { throw "bridge mock object compilation failed" }
& cl.exe @Common /c "$Root\tests\mock_spi\dummy_dispatch.c" "/Fo$DispatchObj"
if ($LASTEXITCODE -ne 0) { throw "dispatch mock object compilation failed" }

$Plugin = Join-Path $Build "stfixest_mock_windows.plugin"
& link.exe /NOLOGO /DLL /INCREMENTAL:NO /OPT:REF /OPT:ICF `
    "/OUT:$Plugin" $StpluginObj $BridgeObj $DispatchObj
if ($LASTEXITCODE -ne 0) { throw "Windows mock plugin link failed" }

$Loader = Join-Path $Build "plugin_loader_test_windows.exe"
& cl.exe @LoaderCommon "$Root\tests\mock_spi\plugin_loader_test_windows.c" "/Fe:$Loader"
if ($LASTEXITCODE -ne 0) { throw "Windows mock loader compilation failed" }
python "$Root\scripts\audit_binary.py" $Plugin | Out-File -Encoding utf8 (Join-Path $Build "audit.json")
if ($LASTEXITCODE -ne 0) { throw "Windows mock plugin audit failed" }
& $Loader $Plugin
if ($LASTEXITCODE -ne 0) { throw "Windows mock plugin load test failed" }
