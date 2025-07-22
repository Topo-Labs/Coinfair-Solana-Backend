@echo off
REM 生产环境启动脚本
chcp 65001 >nul

echo 启动生产环境...

REM 设置环境变量
set CARGO_ENV=production

REM 启动程序
set RUST_LOG=info
cargo run --bin coinfair --release