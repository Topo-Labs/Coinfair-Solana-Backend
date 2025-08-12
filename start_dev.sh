#!/bin/bash
# 开发环境启动脚本

echo "🚀 启动开发环境..."

# 确保 logs 目录存在
mkdir -p logs

# 生成带时间戳的日志文件名（格式：server_YYYYMMDD_HHMMSS.log）
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
LOG_FILE="logs/server_${TIMESTAMP}.log"

# 设置环境变量
export CARGO_ENV=development
export MONGO_DB=coinfair_development


# 启动程序
# RUST_LOG=debug cargo run --bin coinfair 2>&1 | sed 's/\x1b\[[0-9;]*m//g' > "$LOG_FILE" &
RUST_LOG=debug cargo run --bin coinfair