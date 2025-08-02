use crate::auth::{AuthConfig, AuthResponse, Claims, JwtManager, Permission, SolanaLoginRequest, UserInfo, UserTier};
use anyhow::{anyhow, Result};
use bs58;
use chrono::Utc;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::time::Duration as TokioDuration;
use utoipa::ToSchema;

/// Solana签名验证器
pub struct SolanaAuthService {
    jwt_manager: Arc<JwtManager>,
    config: AuthConfig,
    pending_messages: Arc<Mutex<HashMap<String, PendingAuthMessage>>>,
}

/// 待验证的认证消息
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PendingAuthMessage {
    message: String,
    wallet_address: String,
    created_at: u64,
    nonce: String,
}

/// 认证消息生成请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct GenerateAuthMessageRequest {
    pub wallet_address: String,
}

/// 认证消息响应
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthMessageResponse {
    pub message: String,
    pub nonce: String,
    pub expires_at: u64,
}

impl SolanaAuthService {
    pub fn new(jwt_manager: JwtManager, config: AuthConfig) -> Self {
        let service = Self {
            jwt_manager: Arc::new(jwt_manager),
            config,
            pending_messages: Arc::new(Mutex::new(HashMap::new())),
        };

        // 启动清理任务，定期清理过期的待验证消息
        service.start_cleanup_task();
        service
    }

    /// 生成认证消息供前端签名
    pub fn generate_auth_message(&self, wallet_address: &str) -> Result<AuthMessageResponse> {
        let nonce = self.generate_nonce();
        let timestamp = Utc::now().timestamp() as u64;
        let expires_at = timestamp + self.config.solana_auth_message_ttl;

        let message = format!(
            "Sign this message to authenticate with Coinfair:\n\nWallet: {}\nNonce: {}\nTimestamp: {}\n\nThis signature will not trigger any blockchain transaction or cost any gas fees.",
            wallet_address, nonce, timestamp
        );

        let pending_message = PendingAuthMessage {
            message: message.clone(),
            wallet_address: wallet_address.to_string(),
            created_at: timestamp,
            nonce: nonce.clone(),
        };

        // 存储待验证消息
        {
            let mut pending = self.pending_messages.lock().map_err(|_| anyhow!("Failed to acquire lock on pending messages"))?;
            pending.insert(nonce.clone(), pending_message);
        }

        Ok(AuthMessageResponse { message, nonce, expires_at })
    }

    /// 验证Solana钱包签名并生成JWT令牌
    pub async fn authenticate_wallet(&self, request: SolanaLoginRequest) -> Result<AuthResponse> {
        // 验证消息是否存在且未过期
        let pending_message = {
            let mut pending = self.pending_messages.lock().map_err(|_| anyhow!("Failed to acquire lock on pending messages"))?;

            pending
                .remove(&self.extract_nonce_from_message(&request.message)?)
                .ok_or_else(|| anyhow!("Authentication message not found or expired"))?
        };

        // 验证钱包地址匹配
        if pending_message.wallet_address != request.wallet_address {
            return Err(anyhow!("Wallet address mismatch"));
        }

        // 验证消息是否过期
        let now = Utc::now().timestamp() as u64;
        if now > pending_message.created_at + self.config.solana_auth_message_ttl {
            return Err(anyhow!("Authentication message has expired"));
        }

        // 验证消息内容匹配
        if pending_message.message != request.message {
            return Err(anyhow!("Message content mismatch"));
        }

        // 验证Solana签名
        self.verify_solana_signature(&request.wallet_address, &request.message, &request.signature)?;

        // 获取或创建用户权限和等级
        let (permissions, tier) = self.get_user_permissions_and_tier(&request.wallet_address).await?;

        // 生成JWT令牌
        let access_token = self
            .jwt_manager
            .generate_token(&request.wallet_address, Some(&request.wallet_address), permissions.clone(), tier.clone())?;

        Ok(AuthResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: self.config.jwt_expires_in_hours * 3600,
            user: UserInfo {
                user_id: request.wallet_address.clone(),
                wallet_address: Some(request.wallet_address),
                tier,
                permissions,
            },
        })
    }

    /// 验证Solana Ed25519签名
    fn verify_solana_signature(&self, wallet_address: &str, message: &str, signature_str: &str) -> Result<()> {
        // 解码钱包地址（Base58）
        let wallet_bytes = bs58::decode(wallet_address).into_vec().map_err(|e| anyhow!("Invalid wallet address format: {}", e))?;

        if wallet_bytes.len() != 32 {
            return Err(anyhow!("Invalid wallet address length"));
        }

        // 解码签名（Base58）
        let signature_bytes = bs58::decode(signature_str).into_vec().map_err(|e| anyhow!("Invalid signature format: {}", e))?;

        if signature_bytes.len() != 64 {
            return Err(anyhow!("Invalid signature length"));
        }

        // 创建公钥和签名对象
        let public_key =
            VerifyingKey::from_bytes(&wallet_bytes.try_into().map_err(|_| anyhow!("Invalid wallet address length"))?).map_err(|e| anyhow!("Invalid public key: {}", e))?;

        let signature = Signature::from_bytes(&signature_bytes.try_into().map_err(|_| anyhow!("Invalid signature length"))?);

        // 验证签名
        public_key
            .verify(message.as_bytes(), &signature)
            .map_err(|e| anyhow!("Signature verification failed: {}", e))?;

        Ok(())
    }

    /// 获取用户权限和等级（从数据库或默认配置）
    async fn get_user_permissions_and_tier(&self, wallet_address: &str) -> Result<(Vec<String>, UserTier)> {
        // TODO: 从数据库查询用户信息，这里暂时使用默认配置

        // 检查是否是管理员钱包（可以从环境变量或配置中读取）
        let admin_wallets = self.get_admin_wallets();
        if admin_wallets.contains(&wallet_address.to_string()) {
            return Ok((
                vec![
                    Permission::ReadUser.as_str().to_string(),
                    Permission::ReadPool.as_str().to_string(),
                    Permission::ReadPosition.as_str().to_string(),
                    Permission::ReadReward.as_str().to_string(),
                    Permission::CreateUser.as_str().to_string(),
                    Permission::CreatePool.as_str().to_string(),
                    Permission::CreatePosition.as_str().to_string(),
                    Permission::ManageReward.as_str().to_string(),
                    Permission::AdminConfig.as_str().to_string(),
                    Permission::SystemMonitor.as_str().to_string(),
                    Permission::UserManagement.as_str().to_string(),
                ],
                UserTier::Admin,
            ));
        }

        // 默认新用户权限
        Ok((
            vec![
                Permission::ReadUser.as_str().to_string(),
                Permission::ReadPool.as_str().to_string(),
                Permission::ReadPosition.as_str().to_string(),
                Permission::ReadReward.as_str().to_string(),
            ],
            UserTier::Basic,
        ))
    }

    /// 获取管理员钱包列表
    fn get_admin_wallets(&self) -> Vec<String> {
        std::env::var("ADMIN_WALLETS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// 生成随机Nonce
    fn generate_nonce(&self) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();

        // 使用时间戳和随机数生成Nonce
        let random_part: u64 = rand::random();
        format!("{:x}{:x}", timestamp, random_part)
    }

    /// 从消息中提取Nonce
    fn extract_nonce_from_message(&self, message: &str) -> Result<String> {
        for line in message.lines() {
            if line.starts_with("Nonce: ") {
                return Ok(line.strip_prefix("Nonce: ").unwrap_or("").to_string());
            }
        }
        Err(anyhow!("Nonce not found in message"))
    }

    /// 启动清理任务，定期清理过期的待验证消息
    fn start_cleanup_task(&self) {
        let pending_messages = Arc::clone(&self.pending_messages);
        let ttl = self.config.solana_auth_message_ttl;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(TokioDuration::from_secs(60)); // 每分钟清理一次

            loop {
                interval.tick().await;

                if let Ok(mut pending) = pending_messages.lock() {
                    let now = Utc::now().timestamp() as u64;
                    pending.retain(|_, msg| now <= msg.created_at + ttl);
                }
            }
        });
    }

    /// 刷新用户权限（管理员功能）
    pub async fn refresh_user_permissions(&self, wallet_address: &str) -> Result<Vec<String>> {
        let (permissions, _) = self.get_user_permissions_and_tier(wallet_address).await?;
        Ok(permissions)
    }

    /// 验证API密钥签名（用于程序化访问）
    pub fn verify_api_key_signature(&self, api_key: &str, _message: &str, _signature: &str) -> Result<Claims> {
        // 验证API密钥格式的JWT令牌
        let claims = self.jwt_manager.verify_token(api_key)?;

        if !self.jwt_manager.is_api_key_token(api_key)? {
            return Err(anyhow!("Invalid API key format"));
        }

        Ok(claims)
    }
}

/// 签名验证辅助函数
pub struct SignatureUtils;

impl SignatureUtils {
    /// 验证Base58编码的有效性
    pub fn is_valid_base58(input: &str) -> bool {
        bs58::decode(input).into_vec().is_ok()
    }

    /// 验证Solana钱包地址格式
    pub fn is_valid_solana_address(address: &str) -> bool {
        if let Ok(bytes) = bs58::decode(address).into_vec() {
            bytes.len() == 32
        } else {
            false
        }
    }

    /// 生成认证消息模板
    pub fn create_auth_message_template(wallet: &str, nonce: &str, timestamp: u64) -> String {
        format!(
            "Sign this message to authenticate with Coinfair:\n\nWallet: {}\nNonce: {}\nTimestamp: {}\n\nThis signature will not trigger any blockchain transaction or cost any gas fees.",
            wallet, nonce, timestamp
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthConfig;

    fn create_test_config() -> AuthConfig {
        AuthConfig {
            jwt_secret: "test_secret_key_for_solana_auth_testing".to_string(),
            jwt_expires_in_hours: 24,
            solana_auth_message_ttl: 300,
            redis_url: None,
            rate_limit_redis_prefix: "test:ratelimit".to_string(),
            auth_disabled: false,
        }
    }

    #[test]
    fn test_signature_utils() {
        // 测试有效的Solana地址格式
        let valid_address = "11111111111111111111111111111112"; // System Program
        assert!(SignatureUtils::is_valid_base58(valid_address));

        // 测试无效的Base58
        let invalid_base58 = "invalid0OIl";
        assert!(!SignatureUtils::is_valid_base58(invalid_base58));
    }

    #[test]
    fn test_auth_message_template() {
        let message = SignatureUtils::create_auth_message_template("test_wallet", "test_nonce", 1234567890);

        assert!(message.contains("test_wallet"));
        assert!(message.contains("test_nonce"));
        assert!(message.contains("1234567890"));
        assert!(message.contains("Coinfair"));
    }

    #[tokio::test]
    async fn test_generate_auth_message() {
        let config = create_test_config();
        let jwt_manager = JwtManager::new(config.clone());
        let auth_service = SolanaAuthService::new(jwt_manager, config);

        let response = auth_service.generate_auth_message("test_wallet_address").unwrap();

        assert!(!response.message.is_empty());
        assert!(!response.nonce.is_empty());
        assert!(response.expires_at > 0);
        assert!(response.message.contains("test_wallet_address"));
        assert!(response.message.contains(&response.nonce));
    }

    #[tokio::test]
    async fn test_admin_wallet_detection() {
        std::env::set_var("ADMIN_WALLETS", "wallet1,wallet2,wallet3");

        let config = create_test_config();
        let jwt_manager = JwtManager::new(config.clone());
        let auth_service = SolanaAuthService::new(jwt_manager, config);

        let admin_wallets = auth_service.get_admin_wallets();
        assert_eq!(admin_wallets.len(), 3);
        assert!(admin_wallets.contains(&"wallet1".to_string()));
        assert!(admin_wallets.contains(&"wallet2".to_string()));
        assert!(admin_wallets.contains(&"wallet3".to_string()));

        // 清理环境变量
        std::env::remove_var("ADMIN_WALLETS");
    }
}
