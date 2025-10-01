use crate::dtos::solana::cpmm::pool::init_pool_event::{
    ConfigInfo, CreateInitPoolEventRequest, InitPoolEventDetailedResponse, InitPoolEventResponse,
    InitPoolEventsDetailedPageResponse, InitPoolEventsPageResponse, MintInfo, QueryInitPoolEventsRequest, UserPoolStats,
};
use crate::services::solana::cpmm::init_pool_event::init_pool_event_error::InitPoolEventError;
use anyhow::Result;
use database::cpmm::init_pool_event::model::InitPoolEvent;
use database::Database;
use mongodb::bson::{doc, oid::ObjectId, Document};
use mongodb::options::FindOptions;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

pub struct InitPoolEventService {
    db: Arc<Database>,
    rpc_client: Arc<RpcClient>,
}

impl InitPoolEventService {
    pub fn new(db: Arc<Database>, rpc_client: Arc<RpcClient>) -> Self {
        Self { db, rpc_client }
    }

    pub async fn create_event(&self, request: CreateInitPoolEventRequest) -> Result<InitPoolEventResponse> {
        info!("🏗️ 创建池子初始化事件: pool_id={}", request.pool_id);

        // 检查pool_id是否已存在
        if let Ok(Some(_)) = self
            .db
            .init_pool_event_repository
            .find_by_pool_id(&request.pool_id)
            .await
        {
            warn!("⚠️ 池子已存在: {}", request.pool_id);
            return Err(InitPoolEventError::DuplicatePoolId(request.pool_id).into());
        }

        // 检查signature是否已存在
        if let Ok(Some(_)) = self
            .db
            .init_pool_event_repository
            .find_by_signature(&request.signature)
            .await
        {
            warn!("⚠️ 事件signature已存在: {}", request.signature);
            return Err(InitPoolEventError::DuplicateSignature(request.signature).into());
        }

        let event: InitPoolEvent = request.into();
        let created_event = self.db.init_pool_event_repository.insert(event).await?;

        info!("✅ 池子初始化事件创建成功: pool_id={}", created_event.pool_id);
        Ok(created_event.into())
    }

    pub async fn get_event_by_id(&self, id: &str) -> Result<InitPoolEventResponse> {
        debug!("🔍 根据ID查询池子初始化事件: {}", id);

        let object_id = ObjectId::from_str(id).map_err(|_| InitPoolEventError::EventNotFound)?;

        let event = self
            .db
            .init_pool_event_repository
            .find_by_id(&object_id)
            .await?
            .ok_or(InitPoolEventError::EventNotFound)?;

        Ok(event.into())
    }

    pub async fn get_event_by_pool_id(&self, pool_id: &str) -> Result<InitPoolEventResponse> {
        debug!("🔍 根据pool_id查询池子初始化事件: {}", pool_id);

        let event = self
            .db
            .init_pool_event_repository
            .find_by_pool_id(pool_id)
            .await?
            .ok_or(InitPoolEventError::EventNotFound)?;

        Ok(event.into())
    }

    pub async fn get_event_by_signature(&self, signature: &str) -> Result<InitPoolEventResponse> {
        debug!("🔍 根据signature查询池子初始化事件: {}", signature);

        let event = self
            .db
            .init_pool_event_repository
            .find_by_signature(signature)
            .await?
            .ok_or(InitPoolEventError::EventNotFound)?;

        Ok(event.into())
    }

    pub async fn query_events(&self, request: QueryInitPoolEventsRequest) -> Result<InitPoolEventsPageResponse> {
        debug!("🔍 查询池子初始化事件列表");

        let mut filter = Document::new();

        // 处理多个pool_id（英文逗号分隔）
        if let Some(pool_ids) = &request.pool_ids {
            let ids: Vec<String> = pool_ids
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if !ids.is_empty() {
                filter.insert("pool_id", doc! { "$in": ids });
            }
        }

        // 根据池子创建者过滤
        if let Some(pool_creator) = &request.pool_creator {
            filter.insert("pool_creator", pool_creator);
        }

        // 根据LP mint过滤
        if let Some(lp_mint) = &request.lp_mint {
            filter.insert("lp_mint", lp_mint);
        }

        // 根据token_0_mint过滤
        if let Some(token_0_mint) = &request.token_0_mint {
            filter.insert("token_0_mint", token_0_mint);
        }

        // 根据token_1_mint过滤
        if let Some(token_1_mint) = &request.token_1_mint {
            filter.insert("token_1_mint", token_1_mint);
        }

        // 时间范围过滤
        if request.start_time.is_some() || request.end_time.is_some() {
            let mut time_filter = Document::new();
            if let Some(start) = request.start_time {
                // 将 chrono::DateTime 转换为 BSON DateTime
                let bson_datetime = mongodb::bson::DateTime::from_system_time(start.into());
                time_filter.insert("$gte", bson_datetime);
            }
            if let Some(end) = request.end_time {
                // 将 chrono::DateTime 转换为 BSON DateTime
                let bson_datetime = mongodb::bson::DateTime::from_system_time(end.into());
                time_filter.insert("$lte", bson_datetime);
            }
            filter.insert("created_at", time_filter);
        }

        // 分页参数
        let page = request.page.unwrap_or(1).max(1);
        let page_size = request.page_size.unwrap_or(20).min(100);
        let skip = (page - 1) * page_size;

        let options = FindOptions::builder()
            .sort(doc! { "created_at": -1 })
            .skip(skip)
            .limit(page_size as i64)
            .build();

        // 查询数据和总数
        let events = self
            .db
            .init_pool_event_repository
            .find_with_filter(filter.clone(), options)
            .await?;

        let total = self.db.init_pool_event_repository.count_with_filter(filter).await?;

        let total_pages = (total + page_size - 1) / page_size;

        let response_events: Vec<InitPoolEventResponse> = events.into_iter().map(|event| event.into()).collect();

        Ok(InitPoolEventsPageResponse {
            data: response_events,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    pub async fn get_user_pool_stats(&self, pool_creator: &str) -> Result<UserPoolStats> {
        debug!("📊 获取用户池子创建统计: {}", pool_creator);

        // 使用Repository层的聚合查询方法，一次查询获取所有统计数据
        let stats = self
            .db
            .init_pool_event_repository
            .get_user_pool_stats(pool_creator)
            .await?;

        // 转换为Service层的UserPoolStats（注意这里需要类型转换）
        Ok(UserPoolStats {
            total_pools_created: stats.total_pools_created,
            first_pool_created_at: stats.first_pool_created_at,
            latest_pool_created_at: stats.latest_pool_created_at,
        })
    }

    pub async fn delete_event(&self, id: &str) -> Result<bool> {
        info!("🗑️ 删除池子初始化事件: {}", id);

        let object_id = ObjectId::from_str(id).map_err(|_| InitPoolEventError::EventNotFound)?;

        let deleted = self.db.init_pool_event_repository.delete_by_id(&object_id).await?;

        if deleted {
            info!("✅ 池子初始化事件删除成功: {}", id);
        } else {
            warn!("⚠️ 池子初始化事件不存在: {}", id);
        }

        Ok(deleted)
    }

    /// 查询带详细信息的池子初始化事件（包含config和token信息）
    pub async fn query_events_with_details(
        &self,
        request: QueryInitPoolEventsRequest,
    ) -> Result<InitPoolEventsDetailedPageResponse> {
        debug!("🔍 查询带详细信息的池子初始化事件列表");

        // 1. 首先查询事件列表
        let events_page = self.query_events(request).await?;

        if events_page.data.is_empty() {
            debug!("📋 查询结果为空，返回空列表");
            return Ok(InitPoolEventsDetailedPageResponse {
                data: Vec::new(),
                total: events_page.total,
                page: events_page.page,
                page_size: events_page.page_size,
                total_pages: events_page.total_pages,
            });
        }

        // 2. 收集需要查询的ID（去重）
        let config_ids: Vec<String> = events_page
            .data
            .iter()
            .filter_map(|e| e.amm_config.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let mut mint_ids: Vec<String> = Vec::new();
        for event in &events_page.data {
            mint_ids.push(event.token_0_mint.clone());
            mint_ids.push(event.token_1_mint.clone());
        }
        let mint_ids: Vec<String> = mint_ids.into_iter().collect::<std::collections::HashSet<_>>().into_iter().collect();

        debug!(
            "📊 需要查询 {} 个配置ID 和 {} 个代币地址",
            config_ids.len(),
            mint_ids.len()
        );

        // 3. 并发批量查询配置和代币信息
        let (configs_result, tokens_result) = tokio::join!(
            self.db.cpmm_config_repository.get_configs_by_addresses_batch(&config_ids),
            self.db.token_info_repository.find_by_addresses(&mint_ids)
        );

        // 4. 处理查询结果
        let configs = configs_result.unwrap_or_else(|e| {
            warn!("⚠️ 批量查询配置信息失败: {}", e);
            Vec::new()
        });

        let tokens = tokens_result.unwrap_or_else(|e| {
            warn!("⚠️ 批量查询代币信息失败: {}", e);
            Vec::new()
        });

        // 5. 构建HashMap以便快速查找
        let config_map: HashMap<String, ConfigInfo> = configs
            .into_iter()
            .map(|c| {
                (
                    c.config_id.clone(),
                    ConfigInfo {
                        id: c.config_id,
                        index: c.index,
                        protocol_fee_rate: c.protocol_fee_rate,
                        trade_fee_rate: c.trade_fee_rate,
                        fund_fee_rate: c.fund_fee_rate,
                        create_pool_fee: c.create_pool_fee.to_string(),
                        creator_fee_rate: c.creator_fee_rate,
                    },
                )
            })
            .collect();

        let token_map: HashMap<String, MintInfo> = tokens
            .into_iter()
            .map(|t| {
                (
                    t.address.clone(),
                    MintInfo {
                        logo_uri: t.logo_uri,
                        symbol: t.symbol,
                        name: t.name,
                    },
                )
            })
            .collect();

        // 6. 批量查询所有 vault 的余额
        let mut vault_addresses = Vec::new();
        for event in &events_page.data {
            vault_addresses.push(event.token_0_vault.clone());
            vault_addresses.push(event.token_1_vault.clone());
        }

        // 去重
        let vault_addresses: Vec<String> = vault_addresses
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        debug!("📊 需要查询 {} 个 vault 余额", vault_addresses.len());

        // 批量查询 vault 余额
        let vault_balances = self.fetch_vault_balances(&vault_addresses).await;

        // 7. 组装详细事件数据
        let detailed_events: Vec<InitPoolEventDetailedResponse> = events_page
            .data
            .into_iter()
            .map(|event| {
                let config = event
                    .amm_config
                    .as_ref()
                    .and_then(|config_id| config_map.get(config_id).cloned());
                let mint_a = token_map.get(&event.token_0_mint).cloned();
                let mint_b = token_map.get(&event.token_1_mint).cloned();

                if config.is_none() && event.amm_config.is_some() {
                    debug!("⚠️ 未找到配置信息: {}", event.amm_config.as_ref().unwrap());
                }
                if mint_a.is_none() {
                    debug!("⚠️ 未找到Token A信息: {}", event.token_0_mint);
                }
                if mint_b.is_none() {
                    debug!("⚠️ 未找到Token B信息: {}", event.token_1_mint);
                }

                // 获取 vault 余额
                let vault_0_balance = vault_balances.get(&event.token_0_vault);
                let vault_1_balance = vault_balances.get(&event.token_1_vault);

                // 计算 mint amount（考虑小数位数）并格式化为字符串
                let mint_amount_a_raw = vault_0_balance.map(|balance| {
                    *balance as f64 / 10_f64.powi(event.token_0_decimals as i32)
                });
                let mint_amount_b_raw = vault_1_balance.map(|balance| {
                    *balance as f64 / 10_f64.powi(event.token_1_decimals as i32)
                });

                // 格式化为字符串，避免科学计数法
                let mint_amount_a = mint_amount_a_raw.map(|amount| {
                    Self::format_amount(amount, event.token_0_decimals)
                });
                let mint_amount_b = mint_amount_b_raw.map(|amount| {
                    Self::format_amount(amount, event.token_1_decimals)
                });

                // 计算价格（wsol / token）并格式化为字符串（保留8位小数）
                let price = if let (Some(amount_a), Some(amount_b)) = (mint_amount_a_raw, mint_amount_b_raw) {
                    if amount_b > 0.0 {
                        Some(format!("{:.8}", amount_a / amount_b))
                    } else {
                        None
                    }
                } else {
                    None
                };

                // 计算手续费率并格式化为字符串（保留4位小数）
                let fee_rate = config.as_ref().map(|c| {
                    format!("{:.4}", c.protocol_fee_rate as f64 / 10000.0)
                });

                InitPoolEventDetailedResponse {
                    event,
                    config,
                    mint_a,
                    mint_b,
                    mint_amount_a,
                    mint_amount_b,
                    price,
                    fee_rate,
                }
            })
            .collect();

        info!(
            "✅ 查询带详细信息的池子初始化事件成功: 共{}条，其中{}条有配置信息，{}条有Token A信息，{}条有Token B信息",
            detailed_events.len(),
            detailed_events.iter().filter(|e| e.config.is_some()).count(),
            detailed_events.iter().filter(|e| e.mint_a.is_some()).count(),
            detailed_events.iter().filter(|e| e.mint_b.is_some()).count(),
        );

        Ok(InitPoolEventsDetailedPageResponse {
            data: detailed_events,
            total: events_page.total,
            page: events_page.page,
            page_size: events_page.page_size,
            total_pages: events_page.total_pages,
        })
    }

    /// 批量查询 vault 的 token 余额
    async fn fetch_vault_balances(&self, vault_addresses: &[String]) -> HashMap<String, u64> {
        let mut balances = HashMap::new();

        // 解析所有的 vault 地址
        let pubkeys: Vec<_> = vault_addresses
            .iter()
            .filter_map(|addr| {
                Pubkey::from_str(addr)
                    .map_err(|e| {
                        warn!("⚠️ 无效的 vault 地址 {}: {}", addr, e);
                        e
                    })
                    .ok()
            })
            .collect();

        if pubkeys.is_empty() {
            return balances;
        }

        // 批量查询账户信息
        match self.rpc_client.get_multiple_accounts(&pubkeys) {
            Ok(accounts) => {
                for (i, account_option) in accounts.into_iter().enumerate() {
                    if let Some(account) = account_option {
                        // SPL Token 账户的余额在第 64-72 字节（u64 little-endian）
                        if account.data.len() >= 72 {
                            let balance_bytes: [u8; 8] = account.data[64..72].try_into().unwrap_or([0u8; 8]);
                            let balance = u64::from_le_bytes(balance_bytes);
                            balances.insert(vault_addresses[i].clone(), balance);
                        } else {
                            warn!(
                                "⚠️ Vault {} 账户数据长度不足: {} bytes",
                                vault_addresses[i],
                                account.data.len()
                            );
                        }
                    } else {
                        debug!("⚠️ Vault {} 不存在", vault_addresses[i]);
                    }
                }
            }
            Err(e) => {
                error!("❌ 批量查询 vault 余额失败: {}", e);
            }
        }

        debug!("✅ 成功查询 {} 个 vault 余额", balances.len());
        balances
    }

    /// 格式化金额为字符串，避免科学计数法
    fn format_amount(amount: f64, decimals: u8) -> String {
        // 根据小数位数确定格式化精度
        let precision = decimals as usize;

        // 格式化为固定小数位数
        let formatted = format!("{:.precision$}", amount, precision = precision);

        // 移除末尾的零，但保留至少一位小数
        let trimmed = formatted.trim_end_matches('0');

        // 如果全部是零（例如 "0."），保留一位小数
        if trimmed.ends_with('.') {
            format!("{}0", trimmed)
        } else {
            trimmed.to_string()
        }
    }
}
