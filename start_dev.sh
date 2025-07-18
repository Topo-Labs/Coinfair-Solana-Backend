#!/bin/bash
# 开发环境启动脚本

echo "🚀 启动开发环境..."

# 设置环境变量
export CARGO_ENV=development

# 启动程序
RUST_LOG=info cargo run --bin coinfair 