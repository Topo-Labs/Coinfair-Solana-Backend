#!/bin/bash

echo "=== SSH连接调试脚本 ==="
echo "时间: $(date)"
echo

# 1. 检查GitHub Secrets是否正确设置
echo "1. 检查必要的环境变量（在GitHub Actions中运行）"
echo "   AWS_HOST: ${AWS_HOST:-(未设置)}"
echo "   AWS_USER: ${AWS_USER:-(未设置)}"
echo "   AWS_PRIVATE_KEY长度: ${#AWS_PRIVATE_KEY} 字符"
echo

# 2. 测试SSH连接
echo "2. 测试SSH连接"
if [ -n "$AWS_HOST" ] && [ -n "$AWS_USER" ] && [ -n "$AWS_PRIVATE_KEY" ]; then
    # 创建临时私钥文件
    echo "$AWS_PRIVATE_KEY" > /tmp/private_key
    chmod 600 /tmp/private_key
    
    echo "尝试SSH连接到 $AWS_USER@$AWS_HOST"
    
    # 详细的SSH连接测试
    ssh -vvv -i /tmp/private_key -o StrictHostKeyChecking=no -o ConnectTimeout=30 \
        "$AWS_USER@$AWS_HOST" "echo 'SSH连接成功'; whoami; pwd" 2>&1
    
    # 清理临时文件
    rm -f /tmp/private_key
else
    echo "环境变量未正确设置，无法测试SSH连接"
fi

echo
echo "3. 网络连接测试"
if [ -n "$AWS_HOST" ]; then
    echo "测试到 $AWS_HOST 的网络连接："
    ping -c 4 "$AWS_HOST" 2>&1 || echo "ping失败"
    
    echo "测试SSH端口连接："
    nc -zv "$AWS_HOST" 22 2>&1 || echo "SSH端口22连接失败"
fi

echo
echo "=== 调试完成 ==="