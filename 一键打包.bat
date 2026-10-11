@echo off
@chcp 936 >nul
title 汇格 · GridFlow 一键打包

setlocal enabledelayedexpansion
set "SCRIPT_DIR=%~dp0"
cd /d "%SCRIPT_DIR%"

echo ==================================================
echo         汇格 · GridFlow 一键打包工具
echo ==================================================
echo.

echo [1/3] 检查构建环境...

where node >nul 2>&1
if errorlevel 1 goto :no_node

where npm >nul 2>&1
if errorlevel 1 goto :no_npm

where cargo >nul 2>&1
if errorlevel 1 goto :no_cargo

echo [OK] 环境检查通过 (Node.js, npm, Rust/Cargo 均已就绪)
echo.

echo [2/3] 开始全量编译与打包 (包含前端构建与 Rust release 编译)...
echo       此过程可能需要 1-2 分钟，请耐心等待...
echo.

powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%scripts\package.ps1"
if errorlevel 1 goto :build_failed

echo.
echo ==================================================
echo [成功] 汇格 · GridFlow 已成功打包完成！
echo.
echo 输出位置: %SCRIPT_DIR%release\
echo 包含内容: GridFlow.exe (主程序)、duckdb.dll (数据库依赖库) 及完整 ZIP 安装包
echo ==================================================
echo.

set "OPEN_DIR=Y"
set /p "OPEN_DIR=是否立即打开 release 输出文件夹？(Y/n，回车默认打开): "
if /i "!OPEN_DIR!"=="" set "OPEN_DIR=Y"
if /i "!OPEN_DIR!"=="Y" start "" explorer "%SCRIPT_DIR%release"

echo.
echo 打包成功完成，按任意键退出...
pause >nul
exit /b 0

:no_node
echo.
echo [错误] 未检测到 Node.js，请先访问 https://nodejs.org 安装配置。
goto :failed

:no_npm
echo.
echo [错误] 未检测到 npm，请确认 Node.js 环境配置完整。
goto :failed

:no_cargo
echo.
echo [错误] 未检测到 Rust/Cargo，请先访问 https://rustup.rs 安装配置。
goto :failed

:build_failed
echo.
echo ==================================================
echo [错误] 打包过程执行失败，请检查上方的具体错误提示。
echo ==================================================
goto :failed

:failed
echo.
echo 打包未完成，按任意键退出...
pause >nul
exit /b 1
