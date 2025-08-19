use crate::auth::models::{AuthConfig, Claims, UserTier};
use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};

/// JWT令牌管理器
#[derive(Clone)]
pub struct JwtManager {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    config: AuthConfig,
}

impl JwtManager {
    /// 创建新的JWT管理器
    pub fn new(config: AuthConfig) -> Self {
        let encoding_key = EncodingKey::from_secret(config.jwt_secret.as_ref());
        let decoding_key = DecodingKey::from_secret(config.jwt_secret.as_ref());

        Self {
            encoding_key,
            decoding_key,
            config,
        }
    }

    /// 生成JWT令牌
    pub fn generate_token(
        &self,
        user_id: &str,
        wallet_address: Option<&str>,
        permissions: Vec<String>,
        tier: UserTier,
    ) -> Result<String> {
        let now = Utc::now();
        let expires_at = now + Duration::hours(self.config.jwt_expires_in_hours as i64);

        let claims = Claims {
            sub: user_id.to_string(),
            wallet: wallet_address.map(|w| w.to_string()),
            permissions,
            tier,
            exp: expires_at.timestamp() as u64,
            iat: now.timestamp() as u64,
            iss: "coinfair-api".to_string(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| anyhow!("Failed to generate JWT token: {}", e))
    }

    /// 验证JWT令牌
    pub fn verify_token(&self, token: &str) -> Result<Claims> {
        let token_data = decode::<Claims>(token, &self.decoding_key, &Validation::default())
            .map_err(|e| anyhow!("Invalid JWT token: {}", e))?;

        // 检查令牌是否过期
        let now = Utc::now().timestamp() as u64;
        if token_data.claims.exp < now {
            return Err(anyhow!("JWT token has expired"));
        }

        Ok(token_data.claims)
    }

    /// 刷新令牌
    pub fn refresh_token(&self, old_token: &str) -> Result<String> {
        let claims = self.verify_token(old_token)?;

        // 生成新的令牌，保持原有权限和用户信息
        self.generate_token(&claims.sub, claims.wallet.as_deref(), claims.permissions, claims.tier)
    }

    /// 从token中提取用户ID
    pub fn extract_user_id(&self, token: &str) -> Result<String> {
        let claims = self.verify_token(token)?;
        Ok(claims.sub)
    }

    /// 检查令牌是否即将过期(1小时内)
    pub fn is_token_expiring_soon(&self, token: &str) -> Result<bool> {
        let claims = self.verify_token(token)?;
        let now = Utc::now().timestamp() as u64;
        let one_hour = 3600;

        Ok(claims.exp - now <= one_hour)
    }

    /// 生成API密钥令牌(长期有效)
    pub fn generate_api_key_token(
        &self,
        user_id: &str,
        key_id: &str,
        permissions: Vec<String>,
        tier: UserTier,
        expires_in_days: Option<u64>,
    ) -> Result<String> {
        let now = Utc::now();
        let expires_at = match expires_in_days {
            Some(days) => now + Duration::days(days as i64),
            None => now + Duration::days(365), // 默认1年有效
        };

        let claims = Claims {
            sub: format!("api_key:{}:{}", user_id, key_id),
            wallet: None,
            permissions,
            tier,
            exp: expires_at.timestamp() as u64,
            iat: now.timestamp() as u64,
            iss: "coinfair-api-key".to_string(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| anyhow!("Failed to generate API key token: {}", e))
    }

    /// 验证是否为API密钥令牌
    pub fn is_api_key_token(&self, token: &str) -> Result<bool> {
        let claims = self.verify_token(token)?;
        Ok(claims.sub.starts_with("api_key:") && claims.iss == "coinfair-api-key")
    }
}

/// JWT令牌提取器
pub struct TokenExtractor;

impl TokenExtractor {
    /// 从Authorization头部提取Bearer令牌
    pub fn extract_bearer_token(auth_header: Option<&str>) -> Option<String> {
        auth_header
            .and_then(|header| header.strip_prefix("Bearer "))
            .map(|token| token.trim().to_string())
    }

    /// 从请求头中提取API密钥
    pub fn extract_api_key(api_key_header: Option<&str>) -> Option<String> {
        api_key_header.map(|key| key.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> AuthConfig {
        AuthConfig {
            jwt_secret: "test_secret_key_for_jwt_testing_only".to_string(),
            jwt_expires_in_hours: 24,
            solana_auth_message_ttl: 300,
            redis_url: None,
            rate_limit_redis_prefix: "test:ratelimit".to_string(),
            auth_disabled: false,
        }
    }

    #[test]
    fn test_jwt_generation_and_verification() {
        let config = create_test_config();
        let jwt_manager = JwtManager::new(config);

        let permissions = vec!["read:user".to_string(), "create:pool".to_string()];
        let token = jwt_manager
            .generate_token("test_user", Some("test_wallet"), permissions.clone(), UserTier::Premium)
            .unwrap();

        let claims = jwt_manager.verify_token(&token).unwrap();
        assert_eq!(claims.sub, "test_user");
        assert_eq!(claims.wallet, Some("test_wallet".to_string()));
        assert_eq!(claims.permissions, permissions);
        assert_eq!(claims.tier, UserTier::Premium);
    }

    #[test]
    fn test_api_key_token_generation() {
        let config = create_test_config();
        let jwt_manager = JwtManager::new(config);

        let permissions = vec!["read:pool".to_string()];
        let token = jwt_manager
            .generate_api_key_token("user123", "key456", permissions, UserTier::Basic, Some(30))
            .unwrap();

        let claims = jwt_manager.verify_token(&token).unwrap();
        assert!(claims.sub.starts_with("api_key:user123:key456"));
        assert_eq!(claims.iss, "coinfair-api-key");

        let is_api_key = jwt_manager.is_api_key_token(&token).unwrap();
        assert!(is_api_key);
    }

    #[test]
    fn test_bearer_token_extraction() {
        let auth_header = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...";
        let token = TokenExtractor::extract_bearer_token(Some(auth_header));
        assert_eq!(token, Some("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...".to_string()));

        let invalid_header = "Basic dXNlcjpwYXNz";
        let token = TokenExtractor::extract_bearer_token(Some(invalid_header));
        assert_eq!(token, None);
    }

    #[test]
    fn test_token_refresh() {
        let config = create_test_config();
        let jwt_manager = JwtManager::new(config);

        let original_token = jwt_manager
            .generate_token("test_user", None, vec!["read:user".to_string()], UserTier::Basic)
            .unwrap();

        let refreshed_token = jwt_manager.refresh_token(&original_token).unwrap();

        let original_claims = jwt_manager.verify_token(&original_token).unwrap();
        let refreshed_claims = jwt_manager.verify_token(&refreshed_token).unwrap();

        assert_eq!(original_claims.sub, refreshed_claims.sub);
        assert_eq!(original_claims.permissions, refreshed_claims.permissions);
        assert!(refreshed_claims.iat > original_claims.iat);
    }
}
