# 🚀 GitHub Actions 配置指南

本文档详细说明了如何配置Coinfair项目的GitHub Actions CI/CD流水线。

## 📋 概述

我们为Coinfair项目配置了4个主要的GitHub Actions工作流：

- **🔍 CI/CD Pipeline** (`ci.yml`) - 代码质量检查、测试、构建
- **🚀 Production Deployment** (`deploy.yml`) - 自动部署到AWS EC2
- **🗄️ Database Management** (`database.yml`) - 数据库测试和管理
- **🔒 Security Scan** (`security.yml`) - 安全漏洞和密钥泄露扫描

## 🔑 必需的GitHub Secrets配置

在GitHub仓库的 `Settings > Secrets and variables > Actions` 中添加以下secrets：

### AWS部署相关 (🚀 Production Deployment)

| Secret名称 | 描述 | 示例值 | 必需性 |
|-----------|-----|--------|--------|
| `AWS_HOST` | AWS EC2服务器IP地址 | `ec2-43-206-90-117.ap-northeast-1.compute.amazonaws.com` | 必需 |
| `AWS_USER` | AWS EC2登录用户名 | `ubuntu` | 必需 |
| `AWS_PRIVATE_KEY` | AWS EC2 SSH私钥内容 | `-----BEGIN RSA PRIVATE KEY-----\n...` | 必需 |

### Docker Hub (可选，用于Docker镜像构建)

| Secret名称 | 描述 | 示例值 | 必需性 |
|-----------|-----|--------|--------|
| `DOCKER_USERNAME` | Docker Hub用户名 | `coinfair-team` | 可选 |
| `DOCKER_PASSWORD` | Docker Hub密码或Token | `dckr_pat_xxx...` | 可选 |

### 通知系统 (📧 Slack通知)

| Secret名称 | 描述 | 示例值 | 必需性 |
|-----------|-----|--------|--------|
| `SLACK_WEBHOOK_URL` | Slack Webhook URL | `https://hooks.slack.com/services/...` | 推荐 |

## 🛠️ 详细配置步骤

### 1. 配置AWS部署

#### 1.1 生成SSH密钥对（如果还没有）

```bash
# 在本地生成新的SSH密钥对
ssh-keygen -t rsa -b 4096 -f ~/.ssh/coinfair-deploy -C "github-actions@coinfair"

# 将公钥添加到AWS EC2服务器
ssh-copy-id -i ~/.ssh/coinfair-deploy.pub ubuntu@your-ec2-ip
```

#### 1.2 在GitHub中配置Secrets

1. 访问GitHub仓库
2. 进入 `Settings > Secrets and variables > Actions`
3. 点击 `New repository secret`
4. 添加以下secrets：

```yaml
AWS_HOST: ec2-43-206-90-117.ap-northeast-1.compute.amazonaws.com
AWS_USER: ubuntu
AWS_PRIVATE_KEY: |
  -----BEGIN RSA PRIVATE KEY-----
  MIIEpAIBAAKCAQEA1234567890abcdef...
  (完整的私钥内容)
  -----END RSA PRIVATE KEY-----
```

#### 1.3 准备AWS EC2服务器

在EC2服务器上执行以下命令：

```bash
# 创建应用目录
sudo mkdir -p /opt/coinfair
sudo chown ubuntu:ubuntu /opt/coinfair
cd /opt/coinfair

# 安装Docker和Docker Compose
sudo apt update
sudo apt install -y docker.io docker-compose
sudo usermod -aG docker ubuntu

# 创建环境配置文件
cp /path/to/your/.env.production .env

# 启动MongoDB
docker-compose up -d
```

### 2. 配置Slack通知

#### 2.1 创建Slack Webhook

1. 访问 https://api.slack.com/apps
2. 创建新应用或选择现有应用
3. 进入 `Incoming Webhooks`
4. 创建新的Webhook URL
5. 复制Webhook URL

#### 2.2 在GitHub中添加Slack Secret

```yaml
SLACK_WEBHOOK_URL: https://hooks.slack.com/services/T00000000/B00000000/XXXXXXXXXXXXXXXXXXXXXXXX
```

### 3. 配置Docker Hub（可选）

如果需要构建和推送Docker镜像：

```yaml
DOCKER_USERNAME: your-dockerhub-username
DOCKER_PASSWORD: your-dockerhub-token
```

## 🔧 环境配置

### GitHub Actions环境

为了支持不同环境的部署，建议配置GitHub Environment：

1. 进入 `Settings > Environments`
2. 创建以下环境：
   - `production` - 生产环境
   - `staging` - 预发环境

3. 为每个环境配置特定的secrets和保护规则

### 环境特定配置示例

#### Production环境

```yaml
# Environment-specific secrets
AWS_HOST: prod-server.example.com
DATABASE_URL: mongodb://prod-db:27017/coinfair
```

#### Staging环境

```yaml
# Environment-specific secrets  
AWS_HOST: staging-server.example.com
DATABASE_URL: mongodb://staging-db:27017/coinfair_staging
```

## 🚀 工作流触发条件

### CI Pipeline (`ci.yml`)
- **触发时机**: 
  - Push到 `main`, `develop`, `dev_*` 分支
  - Pull Request到 `main`, `develop` 分支
- **忽略文件**: Markdown文档、docs目录

### Production Deployment (`deploy.yml`)
- **触发时机**:
  - Push到 `main` 分支（自动部署生产环境）
  - 创建版本标签 `v*`（自动部署）
  - 手动触发（workflow_dispatch）

### Database Management (`database.yml`)
- **触发时机**:
  - 数据库相关文件变更
  - 手动触发数据库操作

### Security Scan (`security.yml`)
- **触发时机**:
  - 代码Push（自动扫描）
  - 每日定时扫描（UTC 02:00）
  - 手动触发安全扫描

## 🔍 监控和调试

### 查看工作流执行状态

1. 访问GitHub仓库的 `Actions` 标签页
2. 查看具体的工作流执行记录
3. 点击具体的Job查看详细日志

### 常见问题排查

#### 部署失败
```bash
# 检查服务器连接
ssh -i ~/.ssh/coinfair-deploy ubuntu@your-server-ip

# 查看应用日志
tail -f /opt/coinfair/coinfair.log

# 检查服务状态
ps aux | grep coinfair
```

#### 构建失败
- 检查Rust版本兼容性
- 验证依赖项是否正确
- 查看Cargo.lock文件是否需要更新

#### 安全扫描报警
- 查看具体的安全报告artifact
- 更新有漏洞的依赖项
- 检查是否有敏感信息泄露

## 📊 性能优化

### 缓存策略

我们配置了多层缓存来加速构建：

1. **Cargo依赖缓存**: 缓存 `~/.cargo/` 目录
2. **构建产物缓存**: 缓存 `target/` 目录
3. **基于Cargo.lock的智能缓存**: 只有依赖变更时才重新下载

### 并行构建

- 使用矩阵策略并行测试多个Rust版本
- 不同的job并行执行，提高整体流水线速度
- 合理使用 `needs` 关键字控制依赖关系

## 🔒 安全最佳实践

### Secrets管理
- 使用GitHub Secrets存储敏感信息
- 定期轮换密钥和token
- 遵循最小权限原则

### 代码审查
- 配置了 `CODEOWNERS` 文件
- 要求相关专家审查敏感代码变更
- 自动安全扫描集成

### 部署安全
- SSH密钥认证，禁用密码登录
- 生产环境需要手动确认或特定分支触发
- 部署前自动运行安全检查

## 📞 支持和维护

### 团队联系
- **DevOps团队**: 负责CI/CD流水线维护
- **安全团队**: 负责安全扫描和漏洞修复
- **技术负责人**: 负责架构决策和关键变更审查

### 定期维护
- **周**: 检查工作流执行状态
- **月**: 更新依赖项和安全补丁  
- **季**: 评估和优化CI/CD性能

---

## ✅ 配置检查清单

在启用GitHub Actions之前，请确保完成以下步骤：

- [ ] AWS EC2服务器已准备并可SSH连接
- [ ] 配置了所有必需的GitHub Secrets
- [ ] Slack Webhook已配置（用于通知）
- [ ] 服务器上已安装Docker和必要依赖
- [ ] 环境配置文件已放置在正确位置
- [ ] 团队成员已了解工作流触发条件
- [ ] CODEOWNERS文件已更新实际的用户名
- [ ] 测试运行了一次完整的CI/CD流程

完成以上配置后，您的Coinfair项目将拥有完整的自动化CI/CD流水线！🎉