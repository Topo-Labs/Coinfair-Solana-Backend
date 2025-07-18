#!/usr/bin/env bash

# ============ 环境配置管理 ============

# 开发环境启动
dev:
	@echo "🚀 启动开发环境..."
	@CARGO_ENV=development RUST_LOG=debug cargo run --bin coinfair

# 生产环境启动
prod:
	@echo "🚀 启动生产环境..."
	@CARGO_ENV=production RUST_LOG=info cargo run --bin coinfair --release

# 测试环境启动
test:
	@echo "🧪 启动测试环境..."
	@CARGO_ENV=test RUST_LOG=debug cargo test

# 构建项目
build:
	@echo "🔨 构建项目..."
	@cargo build

# 发布构建
build-release:
	@echo "🔨 发布构建..."
	@cargo build --release

# nohup:
#   @echo "Run with nohup..."
#   @nohup  cargo run >> log.txt 2>&1 &


# clean:
# 	@echo "🗑️ \033[36mCleaning the target...\033[36m"
# 	@cargo clean # TODO: Clean database

# check:
# 	@echo "🩺 \033[36mChecking the mongodb...\033[36m"
# 	@sudo bash scripts/docker_check_service.sh

# # Just mongodb now
# stop:
# 	@echo "🚨 \033[36mStopping the mongodb...\033[36m"
# 	@sudo docker-compose stop # 此处不能用down(注意两者区别)

# # API Test
# api-test:
# 	@echo "🧪 \033[36mTesting the API (with hurl)...\033[36m"
# 	@hurl test/api.hurl


# #============= AWS Server ============

login:
	@echo "🔑 \033[36m Login to the AWS server... \033[36m"
	@sudo ssh -i ".aws/hope.pem" ubuntu@ec2-43-206-90-117.ap-northeast-1.compute.amazonaws.com

# # 为AWS服务器编译 aarch64架构的可执行文件
# cross-build:
# 	@echo "Cross build"
# 	@cross build --release --target=aarch64-unknown-linux-gnu

# REMOTE_USER := ubuntu                                # 从你的命令提取
# REMOTE_HOST := ec2-43-206-90-117.ap-northeast-1.compute.amazonaws.com  # 从你的命令提取
# REMOTE_DIR := /home/ubuntu/hope_new                  # 远程目标目录（可根据需要调整）
# LOCAL_DIR := .                                       # 本地项目目录（当前目录）
# SSH_KEY := .aws/hope.pem


# .PHONY: upload
# upload:
# 	@echo "📤 \033[36m Uploading project to ubuntu@ec2-43-206-90-117.ap-northeast-1.compute.amazonaws.com:/home/ubuntu/hope_new... \033[0m"
# 	sudo ssh -i .aws/hope.pem ubuntu@ec2-43-206-90-117.ap-northeast-1.compute.amazonaws.com "mkdir -p /home/ubuntu/hope_new"
# 	sudo rsync -avz --exclude 'target' -e "ssh -i .aws/hope.pem" ./ ubuntu@ec2-43-206-90-117.ap-northeast-1.compute.amazonaws.com:/home/ubuntu/hope_new
# 	@echo "\033[36m Upload complete! \033[0m"
# deploy: 
# 	@echo "Deploy"
# 	@scp target/aarch64-unknown-linux-gnu/release/hope ubuntu@your-aws-ip:~/hope/deploy/

sync:
	@echo "📤 \033[36m Syncing files from the remote server... \033[0m"
	@sudo bash ./script/sync_from_server.sh .aws/hope.pem ubuntu@ec2-43-206-90-117.ap-northeast-1.compute.amazonaws.com:/home/ubuntu/hope_new/crates /Users/orderk/Code/Topo/Temp
	@echo "✅ \033[36m Sync from server complete! \033[0m"
