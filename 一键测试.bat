@echo off
@chcp 936 >nul
setlocal DisableDelayedExpansion
title 汇格 - GridFlow 开发测试

cd /d "%~dp0"
if errorlevel 1 goto :failed

echo ==================================================
echo         汇格 - GridFlow 开发测试
echo ==================================================
echo.

for %%T in (node npm cargo powershell) do (
    where %%T >nul 2>&1
    if errorlevel 1 (
        echo [错误] 未找到 %%T，请先安装项目开发环境后重试。
        echo 按任意键退出...
        pause >nul
        exit /b 1
    )
)

if not exist "node_modules\.bin\tauri.cmd" goto :missing_dependencies
if not exist "node_modules\.bin\vite.cmd" goto :missing_dependencies

echo 正在启动桌面程序，无需打包发布。
echo 首次启动或修改 Rust 后需要编译，请等待桌面窗口出现。
echo 前端修改会自动刷新；Rust 修改会自动编译并重启程序。
echo 运行期间请保留此窗口，结束时可按 Ctrl+C 停止开发服务。
echo.

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\dev.ps1"
if errorlevel 1 goto :failed
exit /b 0

:missing_dependencies
echo [错误] 前端开发依赖尚未安装完整。
echo 请在项目目录运行 npm ci，完成后重新双击本脚本。
goto :failed

:failed
echo.
echo 启动失败，请查看上方错误信息。
echo 按任意键退出...
pause >nul
exit /b 1
