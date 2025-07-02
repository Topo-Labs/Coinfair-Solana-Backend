# Solana Mint Service

一个简单的Solana服务，用于调用referral程序的mint_nft方法。


## 环境配置

### 1. 复制环境变量文件

```bash
cp env.example .env
```

### 2. 编辑.env文件

```bash
# Solana RPC配置
SOLANA_RPC_URL=https://api.devnet.solana.com

# 用户配置
USER_WALLET_ADDRESS=your_wallet_address_here
USER_PRIVATE_KEY=your_private_key_here

# 程序配置
REFERRAL_PROGRAM_ID=REFxcjx4pKym9j5Jzbo9wh92CtYTzHt9fqcjgvZGvUL
NFT_MINT_ADDRESS=your_nft_mint_address_here
```

### 3. 运行示例

```bash
# 构建项目
cargo build
```

### 4.代码结构
```bash
    chains/solana/
    ├── src/
    │   ├── config.rs         # 配置管理（环境变量加载、验证）
    │   ├── mint_service.rs   # NFT铸造服务（核心业务逻辑）
    │   └── lib.rs           # 模块导出
    ├── env.example          # 环境变量模板
    ├── README.md            # 详细使用文档
    └── Cargo.toml          # 依赖配置

1. 配置管理: 使用env来加载重要参数
2. NFT铸造:  调用程序的min_nft方法铸造nft

```
