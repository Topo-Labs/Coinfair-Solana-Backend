#!/bin/bash
# 🚀 GitHub Actions 配置验证脚本

echo "🔍 验证GitHub Actions配置..."

# 检查必需文件是否存在
echo "📋 检查配置文件:"
files=(
    ".github/workflows/ci.yml"
    ".github/workflows/deploy.yml"
    ".github/workflows/database.yml"
    ".github/workflows/security.yml"
    ".github/CODEOWNERS"
    ".github/README.md"
)

for file in "${files[@]}"; do
    if [ -f "$file" ]; then
        echo "✅ $file - 存在"
    else
        echo "❌ $file - 缺失"
    fi
done

# 验证YAML语法
echo -e "\n📋 验证YAML语法:"
python3 -c "
import yaml
import sys

files = [
    '.github/workflows/ci.yml',
    '.github/workflows/deploy.yml', 
    '.github/workflows/database.yml',
    '.github/workflows/security.yml'
]

for f in files:
    try:
        with open(f) as file:
            yaml.safe_load(file)
        print(f'✅ {f} - 语法正确')
    except Exception as e:
        print(f'❌ {f} - 语法错误: {e}')
        sys.exit(1)
"

# 检查项目编译状态
echo -e "\n📋 检查项目编译:"
if cargo check --quiet; then
    echo "✅ 项目编译正常"
else
    echo "❌ 项目编译失败"
    exit 1
fi

# 显示配置统计
echo -e "\n📊 配置统计:"
echo "工作流数量: $(ls -1 .github/workflows/*.yml | wc -l)"
echo "CODEOWNERS规则: $(grep -c '^[^#]' .github/CODEOWNERS || echo 0)"
echo "总配置文件: $(find .github -type f | wc -l)"

echo -e "\n🎉 GitHub Actions配置验证完成!"
echo "📚 查看完整文档: .github/README.md"