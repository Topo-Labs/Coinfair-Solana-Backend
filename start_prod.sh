#!/bin/bash
# 生产环境启动脚本

echo "🚀 启动生产环境..."

# 设置环境变量
export CARGO_ENV=production

# 启动程序
RUST_LOG=info cargo run --bin coinfair --release 