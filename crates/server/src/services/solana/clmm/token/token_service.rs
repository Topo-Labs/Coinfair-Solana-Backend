use crate::dtos::statics::static_dto::{MintListResponse, TokenIdResponse, TokenInfo as DtoTokenInfo};
use database::token_info::{
    StaticTokenInfo, TokenInfo, TokenInfoRepository, TokenListQuery, TokenListResponse, TokenPushRequest,
    TokenPushResponse, TokenStats,
};
use database::Database;
use std::sync::Arc;
use tracing::{error, info, warn};
use utils::AppResult;

/// Token 服务层 - 处理代币相关的业务逻辑
#[derive(Clone)]
pub struct TokenService {
    db: Arc<Database>,
}

impl TokenService {
    /// 创建新的 Token 服务实例
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// 获取代币信息仓库的引用
    fn get_repository(&self) -> &TokenInfoRepository {
        &self.db.token_info_repository
    }

    /// 将StaticTokenInfo转换为DtoTokenInfo
    fn static_to_dto(&self, static_token: StaticTokenInfo) -> DtoTokenInfo {
        DtoTokenInfo {
            address: static_token.address,
            program_id: static_token.program_id,
            name: static_token.name,
            symbol: static_token.symbol,
            decimals: static_token.decimals,
            logo_uri: static_token.logo_uri,
            tags: static_token.tags,
            daily_volume: static_token.daily_volume,
            created_at: static_token.created_at,
            freeze_authority: static_token.freeze_authority,
            mint_authority: static_token.mint_authority,
            permanent_delegate: static_token.permanent_delegate,
            minted_at: static_token.minted_at,
            extensions: static_token.extensions,
        }
    }

    /// 推送代币信息 (创建或更新)
    pub async fn push_token(&self, request: TokenPushRequest) -> AppResult<TokenPushResponse> {
        info!("💾 推送代币信息: {}", request.address);

        // 验证请求数据
        self.validate_push_request(&request)?;

        // 执行推送操作
        let response = self.get_repository().push_token(request).await?;

        if response.success {
            info!("✅ 代币信息推送成功: {} ({})", response.address, response.operation);
        } else {
            warn!("⚠️ 代币信息推送失败: {}", response.message);
        }

        Ok(response)
    }

    /// 验证推送请求数据
    pub fn validate_push_request(&self, request: &TokenPushRequest) -> AppResult<()> {
        // 验证地址格式
        if request.address.len() < 32 || request.address.len() > 44 {
            return Err(utils::AppError::BadRequest("代币地址格式无效".to_string()));
        }

        // 验证符号长度
        if request.symbol.is_empty() || request.symbol.len() > 20 {
            return Err(utils::AppError::BadRequest(
                "代币符号长度必须在1-20字符之间".to_string(),
            ));
        }

        // 验证名称长度
        if request.name.is_empty() || request.name.len() > 100 {
            return Err(utils::AppError::BadRequest(
                "代币名称长度必须在1-100字符之间".to_string(),
            ));
        }

        // 验证小数位数
        if request.decimals > 18 {
            return Err(utils::AppError::BadRequest("代币小数位数不能超过18".to_string()));
        }

        // 验证日交易量
        if let Some(volume) = request.daily_volume {
            if volume < 0.0 {
                return Err(utils::AppError::BadRequest("日交易量不能为负数".to_string()));
            }
        }

        Ok(())
    }

    /// 获取代币列表 (与现有静态接口兼容的格式)
    pub async fn get_token_list(&self, query: Option<TokenListQuery>) -> AppResult<MintListResponse> {
        info!("📋 获取代币列表");

        let query = query.unwrap_or_default();
        let response = self.get_repository().query_tokens(&query).await?;

        // 转换为静态 DTO 格式
        let mint_list: Vec<DtoTokenInfo> = response
            .mint_list
            .into_iter()
            .map(|static_token| self.static_to_dto(static_token))
            .collect();
        let blacklist = response.blacklist;
        let white_list = response.white_list;

        info!("✅ 成功获取代币列表: {} 个代币", mint_list.len());

        Ok(MintListResponse {
            blacklist,
            mint_list,
            white_list,
        })
    }

    /// 获取代币列表 (新格式，包含分页和统计信息)
    pub async fn query_tokens(&self, mut query: TokenListQuery) -> AppResult<TokenListResponse> {
        info!("🔍 查询代币列表: page={:?}, size={:?}", query.page, query.page_size);

        // 处理participate过滤逻辑
        if let Some(participate_wallet) = &query.participate {
            if !participate_wallet.trim().is_empty() {
                info!("🔍 处理参与者过滤: {}", participate_wallet);

                // 从DepositEvent表查询用户参与过的代币地址列表
                let participated_tokens = self
                    .db
                    .deposit_event_repository
                    .find_participated_tokens_by_user(participate_wallet)
                    .await?;

                info!(
                    "✅ 找到用户参与的代币数量: {}，地址: {}",
                    participated_tokens.len(),
                    participated_tokens.join(",")
                );

                if participated_tokens.is_empty() {
                    // 如果用户没有参与任何代币活动，直接返回空结果
                    info!("⚠️ 用户 {} 没有参与任何代币众筹活动", participate_wallet);

                    let empty_response = TokenListResponse {
                        mint_list: Vec::new(),
                        blacklist: Vec::new(),
                        white_list: Vec::new(),
                        pagination: database::token_info::PaginationInfo {
                            current_page: query.page.unwrap_or(1),
                            page_size: query.page_size.unwrap_or(100),
                            total_count: 0,
                            total_pages: 0,
                            has_next: false,
                            has_prev: false,
                        },
                        stats: database::token_info::FilterStats {
                            status_counts: Vec::new(),
                            source_counts: Vec::new(),
                            verification_counts: Vec::new(),
                            tag_counts: Vec::new(),
                        },
                    };

                    return Ok(empty_response);
                } else {
                    // 将参与的代币地址列表转换为逗号分隔的字符串，用于地址过滤
                    let addresses_string = participated_tokens.join(",");
                    query.addresses = Some(addresses_string);

                    info!(
                        "🔍 设置地址过滤: 参与的代币数量={}，地址: {}",
                        participated_tokens.len(),
                        participated_tokens.join(",")
                    );
                }

                // 清除participate参数，避免在repository层处理
                query.participate = None;
            }
        }

        let response = self.get_repository().query_tokens(&query).await?;

        info!(
            "✅ 查询完成: {} 个代币, 总数: {}, 页数: {}/{}",
            response.mint_list.len(),
            response.pagination.total_count,
            response.pagination.current_page,
            response.pagination.total_pages
        );

        Ok(response)
    }

    /// 根据地址获取代币信息
    pub async fn get_token_by_address(&self, address: &str) -> AppResult<Option<DtoTokenInfo>> {
        info!("🔍 查询代币信息: {}", address);

        let token = self.get_repository().find_by_address(address).await?;

        match token {
            Some(token) => {
                info!("✅ 找到代币: {} ({})", token.symbol, token.name);
                let static_token = token.to_static_dto();
                let dto_token = self.static_to_dto(static_token);
                Ok(Some(dto_token))
            }
            None => {
                info!("❌ 未找到代币: {}", address);
                Ok(None)
            }
        }
    }

    /// 根据符号搜索代币
    pub async fn search_tokens_by_symbol(&self, symbol: &str) -> AppResult<Vec<DtoTokenInfo>> {
        info!("🔍 按符号搜索代币: {}", symbol);

        let tokens = self.get_repository().find_by_symbol(symbol).await?;
        let static_tokens: Vec<DtoTokenInfo> = tokens
            .into_iter()
            .map(|t| self.static_to_dto(t.to_static_dto()))
            .collect();

        info!("✅ 找到 {} 个匹配的代币", static_tokens.len());
        Ok(static_tokens)
    }

    /// 搜索代币 (支持名称、符号、地址的模糊匹配)
    pub async fn search_tokens(&self, keyword: &str, limit: Option<i64>) -> AppResult<Vec<DtoTokenInfo>> {
        info!("🔍 搜索代币: keyword={}, limit={:?}", keyword, limit);

        if keyword.trim().is_empty() {
            return Ok(Vec::new());
        }

        let tokens = self.get_repository().search_tokens(keyword, limit).await?;
        let static_tokens: Vec<DtoTokenInfo> = tokens
            .into_iter()
            .map(|t| self.static_to_dto(t.to_static_dto()))
            .collect();

        info!("✅ 搜索完成: 找到 {} 个匹配的代币", static_tokens.len());
        Ok(static_tokens)
    }

    /// 获取热门代币 (按交易量排序)
    pub async fn get_trending_tokens(&self, limit: Option<i64>) -> AppResult<Vec<DtoTokenInfo>> {
        info!("📈 获取热门代币: limit={:?}", limit);

        let tokens = self.get_repository().get_trending_tokens(limit).await?;
        let static_tokens: Vec<DtoTokenInfo> = tokens
            .into_iter()
            .map(|t| self.static_to_dto(t.to_static_dto()))
            .collect();

        info!("✅ 获取热门代币完成: {} 个代币", static_tokens.len());
        Ok(static_tokens)
    }

    /// 获取新上线代币 (按创建时间排序)
    pub async fn get_new_tokens(&self, limit: Option<i64>) -> AppResult<Vec<DtoTokenInfo>> {
        info!("🆕 获取新上线代币: limit={:?}", limit);

        let tokens = self.get_repository().get_new_tokens(limit).await?;
        let static_tokens: Vec<DtoTokenInfo> = tokens
            .into_iter()
            .map(|t| self.static_to_dto(t.to_static_dto()))
            .collect();

        info!("✅ 获取新代币完成: {} 个代币", static_tokens.len());
        Ok(static_tokens)
    }

    /// 更新代币状态
    pub async fn update_token_status(
        &self,
        address: &str,
        status: database::token_info::TokenStatus,
    ) -> AppResult<bool> {
        info!("🔄 更新代币状态: {} -> {:?}", address, status);

        let updated = self.get_repository().update_token_status(address, status).await?;

        if updated {
            info!("✅ 代币状态更新成功: {}", address);
        } else {
            warn!("⚠️ 代币状态更新失败: {} (可能不存在)", address);
        }

        Ok(updated)
    }

    /// 更新代币验证状态
    pub async fn update_token_verification(
        &self,
        address: &str,
        verification: database::token_info::VerificationStatus,
    ) -> AppResult<bool> {
        info!("🔄 更新代币验证状态: {} -> {:?}", address, verification);

        let updated = self
            .get_repository()
            .update_token_verification(address, verification)
            .await?;

        if updated {
            info!("✅ 代币验证状态更新成功: {}", address);
        } else {
            warn!("⚠️ 代币验证状态更新失败: {} (可能不存在)", address);
        }

        Ok(updated)
    }

    /// 批量更新代币交易量
    pub async fn batch_update_volumes(&self, volume_updates: &[(String, f64)]) -> AppResult<u64> {
        info!("🔄 批量更新代币交易量: {} 个代币", volume_updates.len());

        let updated_count = self.get_repository().batch_update_volumes(volume_updates).await?;

        info!("✅ 批量更新完成: 成功更新 {} 个代币的交易量", updated_count);
        Ok(updated_count)
    }

    /// 获取代币统计信息
    pub async fn get_token_stats(&self) -> AppResult<TokenStats> {
        info!("📊 获取代币统计信息");

        let stats = self.get_repository().get_token_stats().await?;

        info!(
            "✅ 统计信息: 总数={}, 活跃={}, 已验证={}, 今日新增={}",
            stats.total_tokens, stats.active_tokens, stats.verified_tokens, stats.today_new_tokens
        );

        Ok(stats)
    }

    /// 删除代币 (管理员功能，谨慎使用)
    pub async fn delete_token(&self, address: &str) -> AppResult<bool> {
        warn!("🗑️ 删除代币: {} (管理员操作)", address);

        let deleted = self.get_repository().delete_token(address).await?;

        if deleted {
            warn!("✅ 代币删除成功: {}", address);
        } else {
            warn!("⚠️ 代币删除失败: {} (可能不存在)", address);
        }

        Ok(deleted)
    }

    /// 验证代币地址格式
    pub fn validate_token_address(&self, address: &str) -> AppResult<()> {
        if address.is_empty() {
            return Err(utils::AppError::BadRequest("代币地址不能为空".to_string()));
        }

        if address.len() < 32 || address.len() > 44 {
            return Err(utils::AppError::BadRequest("代币地址格式无效".to_string()));
        }

        // 简单验证是否为 Base58 字符
        let is_base58 = address
            .chars()
            .all(|c| matches!(c, '1'..='9' | 'A'..='H' | 'J'..='N' | 'P'..='Z' | 'a'..='k' | 'm'..='z'));

        if !is_base58 {
            return Err(utils::AppError::BadRequest("代币地址包含无效字符".to_string()));
        }

        Ok(())
    }

    /// 健康检查 - 验证服务和数据库连接
    pub async fn health_check(&self) -> AppResult<()> {
        // 尝试获取统计信息来验证数据库连接
        match self.get_repository().get_token_stats().await {
            Ok(_) => {
                info!("✅ TokenService 健康检查通过");
                Ok(())
            }
            Err(e) => {
                error!("❌ TokenService 健康检查失败: {}", e);
                Err(e)
            }
        }
    }

    /// 处理来自外部平台的代币推送 (包含额外的业务逻辑)
    pub async fn handle_external_push(&self, request: TokenPushRequest) -> AppResult<TokenPushResponse> {
        info!("🚀 处理外部平台代币推送: {}", request.address);

        // 1. 验证推送请求
        self.validate_push_request(&request)?;

        // 2. 检查是否为重复推送
        if let Some(existing) = self.get_repository().find_by_address(&request.address).await? {
            info!("ℹ️ 发现现有代币记录: {} ({})", existing.symbol, existing.name);

            // 检查是否需要更新
            if self.should_update_token(&existing, &request) {
                info!("🔄 代币信息需要更新");
            } else {
                info!("⏭️ 代币信息无需更新，跳过");
                return Ok(TokenPushResponse {
                    success: true,
                    address: request.address,
                    operation: "skipped".to_string(),
                    message: "代币信息已是最新，无需更新".to_string(),
                    timestamp: chrono::Utc::now(),
                });
            }
        }

        // 3. 执行推送操作
        let response = self.get_repository().push_token(request).await?;

        // 4. 记录推送事件 (可以扩展为发送通知、更新缓存等)
        if response.success {
            self.post_push_actions(&response).await?;
        }

        Ok(response)
    }

    /// 判断是否需要更新代币信息
    fn should_update_token(&self, existing: &TokenInfo, request: &TokenPushRequest) -> bool {
        // 检查关键字段是否有变化
        if existing.name != request.name
            || existing.symbol != request.symbol
            || existing.decimals != request.decimals
            || existing.logo_uri != request.logo_uri
        {
            return true;
        }

        // 检查交易量是否有显著变化 (超过 10%)
        if let Some(new_volume) = request.daily_volume {
            let volume_change = (new_volume - existing.daily_volume).abs();
            let relative_change = if existing.daily_volume > 0.0 {
                volume_change / existing.daily_volume
            } else {
                1.0 // 从0变为非0，认为是显著变化
            };

            if relative_change > 0.1 {
                return true;
            }
        }

        // 检查标签是否有变化
        let empty_tags = Vec::new();
        let new_tags = request.tags.as_ref().unwrap_or(&empty_tags);
        if &existing.tags != new_tags {
            return true;
        }

        false
    }

    /// 推送后的处理操作
    async fn post_push_actions(&self, response: &TokenPushResponse) -> AppResult<()> {
        // 这里可以添加推送后的处理逻辑，比如：
        // - 发送通知
        // - 更新缓存
        // - 触发其他业务流程
        // - 记录审计日志

        info!("📝 执行推送后处理: {} ({})", response.address, response.operation);

        // 示例：如果是新创建的代币，可以触发额外的验证流程
        if response.operation == "created" {
            info!("🆕 新代币创建，触发验证流程: {}", response.address);
            // TODO: 实现新代币验证逻辑
        }

        Ok(())
    }

    /// 根据地址列表批量查询代币信息
    pub async fn get_tokens_by_addresses(&self, addresses: &[String]) -> AppResult<Vec<TokenIdResponse>> {
        info!("🔍 批量查询代币信息: {} 个地址", addresses.len());

        // 验证地址数量限制
        if addresses.len() > 50 {
            return Err(utils::AppError::BadRequest("单次查询地址数量不能超过50个".to_string()));
        }

        // 验证每个地址格式
        for address in addresses {
            self.validate_token_address(address)?;
        }

        // 执行批量查询
        let tokens = self.get_repository().find_by_addresses(addresses).await?;

        // 转换为响应格式
        let responses: Vec<TokenIdResponse> = tokens
            .into_iter()
            .map(|token| TokenIdResponse::from_token_info(self.static_to_dto(token.to_static_dto())))
            .collect();

        info!("✅ 批量查询完成: 找到 {} 个代币", responses.len());
        Ok(responses)
    }
}
