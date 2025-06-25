#!/bin/bash

# 检查参数是否足够
if [ "$#" -ne 3 ]; then
    echo "Usage: $0 <pem_key> <remote_user@remote_host:/remote/path> <local_path>"
    exit 1
fi

# 解析参数
PEM_KEY=$1
REMOTE_PATH=$2
LOCAL_PATH=$3

# 使用 rsync 进行远程复制
rsync -avz -e "ssh -i $PEM_KEY -o StrictHostKeyChecking=no" "$REMOTE_PATH" "$LOCAL_PATH"

# 检查是否成功
if [ $? -eq 0 ]; then
    echo "✅ 文件夹同步完成：$REMOTE_PATH -> $LOCAL_PATH"
else
    echo "❌ 同步失败，请检查连接或路径是否正确"
fi