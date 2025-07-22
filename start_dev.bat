@echo off
REM 开发环境启动脚本
chcp 65001 >nul

echo 启动开发环境...

REM 设置环境变量
set CARGO_ENV=development

REM 启动程序
set RUST_LOG=info
cargo run --bin coinfair