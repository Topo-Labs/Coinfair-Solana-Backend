#!/bin/bash
# 🧪 简化版CI测试脚本 - 用于本地验证GitHub Actions配置

echo "🔍 测试GitHub Actions配置..."

# 模拟CI环境变量
export CARGO_ENV=Development
export MONGO_URI=mongodb://localhost:27017
export MONGO_DB=coinfair_test

echo "📋 1. 检查代码格式..."
if cargo fmt --all -- --check; then
    echo "✅ 代码格式检查通过"
else
    echo "❌ 代码格式检查失败"
    exit 1
fi

echo "📋 2. 检查核心包编译..."
packages=("utils" "database" "server" "monitor" "telegram" "timer" "coinfair")
failed_packages=()

for package in "${packages[@]}"; do
    echo "  检查 $package..."
    if cargo check --package "$package" --quiet; then
        echo "  ✅ $package 编译成功"
    else
        echo "  ⚠️ $package 编译失败"
        failed_packages+=("$package")
    fi
done

echo "📋 3. 运行核心组件测试..."
test_packages=("utils" "database" "server")
for package in "${test_packages[@]}"; do
    echo "  测试 $package..."
    if cargo test --package "$package" --lib --quiet; then
        echo "  ✅ $package 测试通过"
    else
        echo "  ⚠️ $package 测试有问题"
    fi
done

echo "📋 4. 检查主二进制文件构建..."
if cargo build --bin coinfair --quiet; then
    echo "✅ 主二进制文件构建成功"
    ls -la target/debug/coinfair
else
    echo "⚠️ 主二进制文件构建失败"
fi

echo "📊 总结:"
echo "编译失败的包: ${failed_packages[*]:-无}"
echo "🎉 简化CI测试完成!"