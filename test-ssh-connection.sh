#!/bin/bash

echo "🔧 SSH连接测试工具"
echo "=================="
echo

# 从GitHub Secrets获取连接信息（需要手动设置）
read -p "请输入服务器地址 (AWS_HOST): " AWS_HOST
read -p "请输入用户名 (AWS_USER): " AWS_USER
read -p "请输入私钥文件路径: " PRIVATE_KEY_PATH

if [ ! -f "$PRIVATE_KEY_PATH" ]; then
    echo "❌ 私钥文件不存在: $PRIVATE_KEY_PATH"
    exit 1
fi

echo
echo "🔍 开始连接测试..."
echo "目标服务器: $AWS_USER@$AWS_HOST"
echo

# 1. 测试网络连接
echo "1️⃣ 测试网络连接..."
if ping -c 2 "$AWS_HOST" >/dev/null 2>&1; then
    echo "✅ ping成功"
else
    echo "❌ ping失败"
fi

# 2. 测试SSH端口
echo "2️⃣ 测试SSH端口..."
if nc -zv "$AWS_HOST" 22 >/dev/null 2>&1; then
    echo "✅ SSH端口22可达"
else
    echo "❌ SSH端口22不可达"
fi

# 3. 验证私钥格式
echo "3️⃣ 验证私钥格式..."
if head -1 "$PRIVATE_KEY_PATH" | grep -q "BEGIN"; then
    echo "✅ 私钥格式正确"
else
    echo "❌ 私钥格式异常"
fi

# 4. 测试SSH连接（详细模式）
echo "4️⃣ 测试SSH连接（详细模式）..."
echo "执行命令: ssh -vvv -i $PRIVATE_KEY_PATH -o StrictHostKeyChecking=no -o ConnectTimeout=30 $AWS_USER@$AWS_HOST 'echo 连接成功; hostname; whoami'"
echo

ssh -vvv -i "$PRIVATE_KEY_PATH" -o StrictHostKeyChecking=no -o ConnectTimeout=30 \
    "$AWS_USER@$AWS_HOST" "echo '✅ SSH连接成功'; hostname; whoami; pwd"

ssh_exit_code=$?

echo
echo "📋 测试结果总结:"
if [ $ssh_exit_code -eq 0 ]; then
    echo "✅ SSH连接测试成功！"
    echo "💡 GitHub Actions应该能够正常连接"
else
    echo "❌ SSH连接测试失败 (退出码: $ssh_exit_code)"
    echo "💡 建议检查以下项目:"
    echo "   - 服务器是否正在运行"
    echo "   - 私钥是否正确"
    echo "   - 服务器上的公钥是否存在"
    echo "   - 网络防火墙规则"
    echo "   - SSH服务配置"
fi

echo
echo "🔧 如果需要检查服务器状态，请在服务器上运行:"
echo "   sudo systemctl status ssh"
echo "   cat ~/.ssh/authorized_keys"
echo "   sudo tail -f /var/log/auth.log"