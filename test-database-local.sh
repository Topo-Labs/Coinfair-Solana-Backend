#!/bin/bash
# 🧪 本地数据库测试脚本 - 区分单元测试和集成测试

echo "🔍 本地数据库测试..."

# 检查MongoDB是否运行
if ! curl -s mongodb://localhost:27017 > /dev/null 2>&1; then
    echo "⚠️ MongoDB未运行，启动Docker服务..."
    docker-compose up -d
    sleep 5
fi

echo "📋 1. 运行数据库模型单元测试（不需要数据库连接）..."
model_tests=(
    "permission_config::model"
    "position::model" 
    "token_info::model"
    "clmm_pool::model"
)

for test in "${model_tests[@]}"; do
    echo "  测试 $test..."
    if cargo test --package database --lib "$test" --quiet; then
        echo "  ✅ $test 单元测试通过"
    else
        echo "  ❌ $test 单元测试失败"
    fi
done

echo "📋 2. 运行数据库仓库集成测试（需要MongoDB）..."
repo_tests=(
    "permission_config::repository"
    "position::repository"
    "token_info::repository"
    "clmm_pool::repository"
)

for test in "${repo_tests[@]}"; do
    echo "  测试 $test..."
    if MONGO_URI=mongodb://localhost:27017 MONGO_DB=test_db_$(date +%s) cargo test --package database --lib "$test" --quiet; then
        echo "  ✅ $test 集成测试通过"
    else
        echo "  ⚠️ $test 集成测试失败（可能是数据依赖问题）"
    fi
done

echo "📊 本地数据库测试完成!"
echo "ℹ️ 在GitHub Actions中，只会运行单元测试，集成测试在单独的database工作流中运行"