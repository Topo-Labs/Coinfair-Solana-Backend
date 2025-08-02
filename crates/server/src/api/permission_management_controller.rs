use crate::auth::SolanaMiddlewareBuilder; // 添加中间件构建器导入
use crate::auth::{AuthUser, GlobalSolanaPermissionConfig, Permission, SolanaApiAction, SolanaApiPermissionConfig, SolanaPermissionPolicy, UserTier};
use crate::services::Services;
use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    middleware, // 添加middleware导入
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, warn};
use utoipa::ToSchema;

/// 权限管理控制器
pub struct PermissionManagementController;

impl PermissionManagementController {
    pub fn routes() -> Router {
        Router::new()
            // 全局配置管理
            .route("/global/config", get(get_global_config))
            .route("/global/config", put(update_global_config))
            .route("/global/toggle-read", post(toggle_global_read))
            .route("/global/toggle-write", post(toggle_global_write))
            .route("/global/emergency-shutdown", post(emergency_shutdown))
            .route("/global/maintenance-mode", post(toggle_maintenance_mode))
            // API配置管理
            .route("/api/configs", get(get_all_api_configs))
            .route("/api/configs/stats", get(get_api_configs_stats))
            .route("/api/configs/category/:category", get(get_api_configs_by_category))
            .route("/api/config/:endpoint", get(get_api_config))
            .route("/api/config/:endpoint", put(update_api_config))
            .route("/api/config/:endpoint", delete(delete_api_config))
            .route("/api/configs/batch", put(batch_update_api_configs))
            // 权限测试
            .route("/test/permission", post(test_permission))
            // 日志和审计
            .route("/logs", get(get_permission_logs))
            .route("/logs/operator/:operator_id", get(get_logs_by_operator))
            .route("/logs/target/:target_type/:target_id", get(get_logs_by_target))
            // 配置重载
            .route("/reload", post(reload_configuration))
            // 🟢 添加管理员认证中间件
            .layer(middleware::from_fn(Self::apply_admin_auth))
    }

    /// 应用管理员认证中间件
    async fn apply_admin_auth(
        Extension(solana_middleware): Extension<Arc<SolanaMiddlewareBuilder>>,
        request: axum::extract::Request,
        next: axum::middleware::Next,
    ) -> Result<axum::response::Response, axum::http::StatusCode> {
        let middleware_fn = solana_middleware.solana_auth();
        middleware_fn(request, next).await
    }
}

// ==================== DTOs ====================

/// 全局配置更新请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateGlobalConfigRequest {
    pub global_read_enabled: bool,
    pub global_write_enabled: bool,
    pub default_read_policy: SolanaPermissionPolicy,
    pub default_write_policy: SolanaPermissionPolicy,
    pub emergency_shutdown: bool,
    pub maintenance_mode: bool,
}

/// 权限开关请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct TogglePermissionRequest {
    pub enabled: bool,
    pub reason: Option<String>,
}

/// API配置更新请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateApiConfigRequest {
    pub name: String,
    pub category: String,
    pub read_policy: SolanaPermissionPolicy,
    pub write_policy: SolanaPermissionPolicy,
    pub enabled: bool,
}

/// 批量API配置更新请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchUpdateApiConfigsRequest {
    pub configs: HashMap<String, UpdateApiConfigRequest>,
}

/// 权限测试请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct TestPermissionRequest {
    pub endpoint: String,
    pub action: SolanaApiAction,
    pub user_tier: UserTier,
    pub permissions: Vec<String>,
}

/// 权限测试响应
#[derive(Debug, Serialize, ToSchema)]
pub struct TestPermissionResponse {
    pub allowed: bool,
    pub reason: Option<String>,
    pub applied_policy: String,
    pub global_config: GlobalConfigSummary,
}

/// 全局配置摘要
#[derive(Debug, Serialize, ToSchema)]
pub struct GlobalConfigSummary {
    pub global_read_enabled: bool,
    pub global_write_enabled: bool,
    pub emergency_shutdown: bool,
    pub maintenance_mode: bool,
    pub version: u64,
}

/// API配置统计响应
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiConfigStatsResponse {
    pub total_configs: u64,
    pub enabled_configs: u64,
    pub disabled_configs: u64,
    pub category_stats: HashMap<String, u64>,
}

/// 权限日志查询参数
#[derive(Debug, Deserialize, ToSchema)]
pub struct PermissionLogQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub operation_type: Option<String>,
    pub target_type: Option<String>,
}

/// 权限日志响应
#[derive(Debug, Serialize, ToSchema)]
pub struct PermissionLogResponse {
    pub logs: Vec<PermissionLogEntry>,
    pub total_count: u64,
    pub page: u64,
    pub page_size: u64,
}

/// 权限日志条目
#[derive(Debug, Serialize, ToSchema)]
pub struct PermissionLogEntry {
    pub id: String,
    pub operation_type: String,
    pub target_type: String,
    pub target_id: String,
    pub operator_id: String,
    pub operator_wallet: Option<String>,
    pub operation_time: u64,
    pub reason: Option<String>,
    pub before_config: Option<String>,
    pub after_config: Option<String>,
}

/// 通用API响应
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: String,
    pub timestamp: u64,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: "操作成功".to_string(),
            timestamp: chrono::Utc::now().timestamp() as u64,
        }
    }

    pub fn error(message: String) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            message,
            timestamp: chrono::Utc::now().timestamp() as u64,
        }
    }
}

// ==================== 控制器处理函数 ====================

/// 获取全局配置
#[utoipa::path(
    get,
    path = "/api/v1/admin/permissions/global/config",
    responses(
        (status = 200, description = "获取全局配置成功", body = ApiResponse<GlobalSolanaPermissionConfig>),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 500, description = "服务器错误")
    ),
    tag = "权限管理"
)]
pub async fn get_global_config(
    Extension(services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<ApiResponse<GlobalSolanaPermissionConfig>>, (StatusCode, Json<ApiResponse<()>>)> {
    // 检查管理员权限
    if !auth_user.is_admin() {
        warn!("Non-admin user {} attempted to access global config", auth_user.user_id);
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    match services.solana_permission.get_global_config().await {
        Ok(config) => {
            info!("Admin {} retrieved global permission config", auth_user.user_id);
            Ok(Json(ApiResponse::success(config)))
        }
        Err(e) => {
            error!("Failed to get global config: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::error("获取全局配置失败".to_string()))))
        }
    }
}

/// 更新全局配置
#[utoipa::path(
    put,
    path = "/api/v1/admin/permissions/global/config",
    request_body = UpdateGlobalConfigRequest,
    responses(
        (status = 200, description = "更新全局配置成功", body = ApiResponse<GlobalSolanaPermissionConfig>),
        (status = 400, description = "请求参数错误"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 500, description = "服务器错误")
    ),
    tag = "权限管理"
)]
pub async fn update_global_config(
    Extension(services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
    Json(request): Json<UpdateGlobalConfigRequest>,
) -> Result<Json<ApiResponse<GlobalSolanaPermissionConfig>>, (StatusCode, Json<ApiResponse<()>>)> {
    // 检查管理员权限
    if !auth_user.is_admin() {
        warn!("Non-admin user {} attempted to update global config", auth_user.user_id);
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    // 获取当前配置
    let mut current_config = match services.solana_permission.get_global_config().await {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to get current global config: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::error("获取当前配置失败".to_string()))));
        }
    };

    // 更新配置
    current_config.global_read_enabled = request.global_read_enabled;
    current_config.global_write_enabled = request.global_write_enabled;
    current_config.emergency_shutdown = request.emergency_shutdown;
    current_config.maintenance_mode = request.maintenance_mode;
    current_config.version += 1;
    current_config.last_updated = chrono::Utc::now().timestamp() as u64;
    current_config.updated_by = auth_user.user_id.clone();

    match services.solana_permission.update_global_config(current_config.clone()).await {
        Ok(_) => {
            info!("Admin {} updated global permission config", auth_user.user_id);
            Ok(Json(ApiResponse::success(current_config)))
        }
        Err(e) => {
            error!("Failed to update global config: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::error("更新全局配置失败".to_string()))))
        }
    }
}

/// 切换全局读取权限
#[utoipa::path(
    post,
    path = "/api/v1/admin/permissions/global/toggle-read",
    request_body = TogglePermissionRequest,
    responses(
        (status = 200, description = "切换成功", body = ApiResponse<String>),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 500, description = "服务器错误")
    ),
    tag = "权限管理"
)]
pub async fn toggle_global_read(
    Extension(services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
    Json(request): Json<TogglePermissionRequest>,
) -> Result<Json<ApiResponse<String>>, (StatusCode, Json<ApiResponse<()>>)> {
    // 检查管理员权限
    if !auth_user.is_admin() {
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    match services.solana_permission.toggle_global_read(request.enabled).await {
        Ok(_) => {
            let message = format!("全局读取权限已{}", if request.enabled { "启用" } else { "禁用" });
            info!("Admin {} toggled global read permission to {}", auth_user.user_id, request.enabled);
            Ok(Json(ApiResponse::success(message)))
        }
        Err(e) => {
            error!("Failed to toggle global read permission: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::error("操作失败".to_string()))))
        }
    }
}

/// 切换全局写入权限
#[utoipa::path(
    post,
    path = "/api/v1/admin/permissions/global/toggle-write",
    request_body = TogglePermissionRequest,
    responses(
        (status = 200, description = "切换成功", body = ApiResponse<String>),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 500, description = "服务器错误")
    ),
    tag = "权限管理"
)]
pub async fn toggle_global_write(
    Extension(services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
    Json(request): Json<TogglePermissionRequest>,
) -> Result<Json<ApiResponse<String>>, (StatusCode, Json<ApiResponse<()>>)> {
    // 检查管理员权限
    if !auth_user.is_admin() {
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    match services.solana_permission.toggle_global_write(request.enabled).await {
        Ok(_) => {
            let message = format!("全局写入权限已{}", if request.enabled { "启用" } else { "禁用" });
            info!("Admin {} toggled global write permission to {}", auth_user.user_id, request.enabled);
            Ok(Json(ApiResponse::success(message)))
        }
        Err(e) => {
            error!("Failed to toggle global write permission: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::error("操作失败".to_string()))))
        }
    }
}

/// 紧急停用
#[utoipa::path(
    post,
    path = "/api/v1/admin/permissions/global/emergency-shutdown",
    request_body = TogglePermissionRequest,
    responses(
        (status = 200, description = "紧急停用操作成功", body = ApiResponse<String>),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 500, description = "服务器错误")
    ),
    tag = "权限管理"
)]
pub async fn emergency_shutdown(
    Extension(services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
    Json(request): Json<TogglePermissionRequest>,
) -> Result<Json<ApiResponse<String>>, (StatusCode, Json<ApiResponse<()>>)> {
    // 检查管理员权限
    if !auth_user.is_admin() {
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    match services.solana_permission.emergency_shutdown(request.enabled).await {
        Ok(_) => {
            let message = if request.enabled {
                "🚨 系统已进入紧急停用状态".to_string()
            } else {
                "✅ 紧急停用状态已解除".to_string()
            };

            if request.enabled {
                error!("🚨 EMERGENCY SHUTDOWN activated by admin {}", auth_user.user_id);
            } else {
                info!("Emergency shutdown deactivated by admin {}", auth_user.user_id);
            }

            Ok(Json(ApiResponse::success(message)))
        }
        Err(e) => {
            error!("Failed to toggle emergency shutdown: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::error("操作失败".to_string()))))
        }
    }
}

/// 切换维护模式
#[utoipa::path(
    post,
    path = "/api/v1/admin/permissions/global/maintenance-mode",
    request_body = TogglePermissionRequest,
    responses(
        (status = 200, description = "维护模式切换成功", body = ApiResponse<String>),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 500, description = "服务器错误")
    ),
    tag = "权限管理"
)]
pub async fn toggle_maintenance_mode(
    Extension(services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
    Json(request): Json<TogglePermissionRequest>,
) -> Result<Json<ApiResponse<String>>, (StatusCode, Json<ApiResponse<()>>)> {
    // 检查管理员权限
    if !auth_user.is_admin() {
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    match services.solana_permission.toggle_maintenance_mode(request.enabled).await {
        Ok(_) => {
            let message = format!("维护模式已{}", if request.enabled { "开启" } else { "关闭" });
            info!("Admin {} toggled maintenance mode to {}", auth_user.user_id, request.enabled);
            Ok(Json(ApiResponse::success(message)))
        }
        Err(e) => {
            error!("Failed to toggle maintenance mode: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::error("操作失败".to_string()))))
        }
    }
}

/// 获取所有API配置
#[utoipa::path(
    get,
    path = "/api/v1/admin/permissions/api/configs",
    responses(
        (status = 200, description = "获取API配置成功", body = ApiResponse<HashMap<String, SolanaApiPermissionConfig>>),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 500, description = "服务器错误")
    ),
    tag = "权限管理"
)]
pub async fn get_all_api_configs(
    Extension(services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<ApiResponse<HashMap<String, SolanaApiPermissionConfig>>>, (StatusCode, Json<ApiResponse<()>>)> {
    // 检查管理员权限
    if !auth_user.is_admin() {
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    match services.solana_permission.get_all_api_configs().await {
        Ok(configs) => {
            info!("Admin {} retrieved all API configs", auth_user.user_id);
            Ok(Json(ApiResponse::success(configs)))
        }
        Err(e) => {
            error!("Failed to get API configs: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::error("获取API配置失败".to_string()))))
        }
    }
}

/// 获取API配置统计
#[utoipa::path(
    get,
    path = "/api/v1/admin/permissions/api/configs/stats",
    responses(
        (status = 200, description = "获取统计信息成功", body = ApiResponse<ApiConfigStatsResponse>),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 500, description = "服务器错误")
    ),
    tag = "权限管理"
)]
pub async fn get_api_configs_stats(
    Extension(services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<ApiResponse<ApiConfigStatsResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    // 检查管理员权限
    if !auth_user.is_admin() {
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    match services.solana_permission.get_permission_stats().await {
        Ok(stats) => {
            let response = ApiConfigStatsResponse {
                total_configs: stats.total_apis as u64,
                enabled_configs: stats.enabled_apis as u64,
                disabled_configs: stats.disabled_apis as u64,
                category_stats: HashMap::new(), // TODO: 实现分类统计
            };
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to get API config stats: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::error("获取统计信息失败".to_string()))))
        }
    }
}

/// 权限测试
#[utoipa::path(
    post,
    path = "/api/v1/admin/permissions/test/permission",
    request_body = TestPermissionRequest,
    responses(
        (status = 200, description = "权限测试完成", body = ApiResponse<TestPermissionResponse>),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 500, description = "服务器错误")
    ),
    tag = "权限管理"
)]
pub async fn test_permission(
    Extension(services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
    Json(request): Json<TestPermissionRequest>,
) -> Result<Json<ApiResponse<TestPermissionResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    // 检查管理员权限
    if !auth_user.is_admin() {
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    // 构造测试用户
    use std::collections::HashSet;
    let mut test_permissions = HashSet::new();
    for perm_str in &request.permissions {
        if let Some(permission) = Permission::from_str(perm_str) {
            test_permissions.insert(permission);
        }
    }

    let test_user = AuthUser {
        user_id: "test_user".to_string(),
        wallet_address: None,
        tier: request.user_tier.clone(),
        permissions: test_permissions,
    };

    // 执行权限测试
    let test_result = services.solana_permission.check_api_permission(&request.endpoint, &request.action, &test_user).await;

    // 获取全局配置信息
    let global_config = match services.solana_permission.get_global_config().await {
        Ok(config) => GlobalConfigSummary {
            global_read_enabled: config.global_read_enabled,
            global_write_enabled: config.global_write_enabled,
            emergency_shutdown: config.emergency_shutdown,
            maintenance_mode: config.maintenance_mode,
            version: config.version,
        },
        Err(_) => GlobalConfigSummary {
            global_read_enabled: false,
            global_write_enabled: false,
            emergency_shutdown: true,
            maintenance_mode: true,
            version: 0,
        },
    };

    let response = TestPermissionResponse {
        allowed: test_result.is_ok(),
        reason: test_result.err(),
        applied_policy: format!("{:?}", request.action), // TODO: 返回实际应用的策略
        global_config,
    };

    info!("Admin {} performed permission test for endpoint {}", auth_user.user_id, request.endpoint);
    Ok(Json(ApiResponse::success(response)))
}

/// 重载配置
#[utoipa::path(
    post,
    path = "/api/v1/admin/permissions/reload",
    responses(
        (status = 200, description = "配置重载成功", body = ApiResponse<String>),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 500, description = "服务器错误")
    ),
    tag = "权限管理"
)]
pub async fn reload_configuration(
    Extension(services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<ApiResponse<String>>, (StatusCode, Json<ApiResponse<()>>)> {
    // 检查管理员权限
    if !auth_user.is_admin() {
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    match services.solana_permission.reload_configuration().await {
        Ok(_) => {
            info!("Admin {} reloaded permission configuration", auth_user.user_id);
            Ok(Json(ApiResponse::success("配置重载完成".to_string())))
        }
        Err(e) => {
            error!("Failed to reload configuration: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::error("配置重载失败".to_string()))))
        }
    }
}

// 其他处理函数（get_api_config, update_api_config, 等）将在下一部分实现...

/// 获取特定API配置
pub async fn get_api_config(
    Extension(services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
    Path(endpoint): Path<String>,
) -> Result<Json<ApiResponse<SolanaApiPermissionConfig>>, (StatusCode, Json<ApiResponse<()>>)> {
    if !auth_user.is_admin() {
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    let decoded_endpoint = urlencoding::decode(&endpoint)
        .map_err(|_| (StatusCode::BAD_REQUEST, Json(ApiResponse::<()>::error("端点路径解码失败".to_string()))))?
        .to_string();

    match services.solana_permission.get_api_config(&decoded_endpoint).await {
        Ok(Some(config)) => Ok(Json(ApiResponse::success(config))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(ApiResponse::<()>::error("API配置未找到".to_string())))),
        Err(e) => {
            error!("Failed to get API config: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::error("获取API配置失败".to_string()))))
        }
    }
}

/// 更新特定API配置
pub async fn update_api_config(
    Extension(services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
    Path(endpoint): Path<String>,
    Json(request): Json<UpdateApiConfigRequest>,
) -> Result<Json<ApiResponse<SolanaApiPermissionConfig>>, (StatusCode, Json<ApiResponse<()>>)> {
    if !auth_user.is_admin() {
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    let decoded_endpoint = urlencoding::decode(&endpoint)
        .map_err(|_| (StatusCode::BAD_REQUEST, Json(ApiResponse::<()>::error("端点路径解码失败".to_string()))))?
        .to_string();

    let updated_config = SolanaApiPermissionConfig {
        endpoint: decoded_endpoint.clone(),
        name: request.name,
        category: request.category,
        read_policy: request.read_policy,
        write_policy: request.write_policy,
        enabled: request.enabled,
        created_at: chrono::Utc::now().timestamp() as u64,
        updated_at: chrono::Utc::now().timestamp() as u64,
    };

    match services.solana_permission.update_api_config(decoded_endpoint, updated_config.clone()).await {
        Ok(_) => {
            info!("Admin {} updated API config for {}", auth_user.user_id, updated_config.endpoint);
            Ok(Json(ApiResponse::success(updated_config)))
        }
        Err(e) => {
            error!("Failed to update API config: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::error("更新API配置失败".to_string()))))
        }
    }
}

/// 删除API配置
pub async fn delete_api_config(
    Extension(services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
    Path(endpoint): Path<String>,
) -> Result<Json<ApiResponse<String>>, (StatusCode, Json<ApiResponse<()>>)> {
    if !auth_user.is_admin() {
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    let decoded_endpoint = urlencoding::decode(&endpoint)
        .map_err(|_| (StatusCode::BAD_REQUEST, Json(ApiResponse::<()>::error("端点路径解码失败".to_string()))))?
        .to_string();

    // 注意：这里实际上不会删除配置，而是禁用它
    match services.solana_permission.get_api_config(&decoded_endpoint).await {
        Ok(Some(mut config)) => {
            config.enabled = false;
            config.updated_at = chrono::Utc::now().timestamp() as u64;

            match services.solana_permission.update_api_config(decoded_endpoint.clone(), config).await {
                Ok(_) => {
                    info!("Admin {} disabled API config for {}", auth_user.user_id, decoded_endpoint);
                    Ok(Json(ApiResponse::success("API配置已禁用".to_string())))
                }
                Err(e) => {
                    error!("Failed to disable API config: {}", e);
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::error("禁用API配置失败".to_string()))))
                }
            }
        }
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(ApiResponse::<()>::error("API配置未找到".to_string())))),
        Err(e) => {
            error!("Failed to get API config for deletion: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::error("操作失败".to_string()))))
        }
    }
}

/// 批量更新API配置
pub async fn batch_update_api_configs(
    Extension(services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
    Json(request): Json<BatchUpdateApiConfigsRequest>,
) -> Result<Json<ApiResponse<String>>, (StatusCode, Json<ApiResponse<()>>)> {
    if !auth_user.is_admin() {
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    let mut configs_to_update = HashMap::new();

    for (endpoint, update_request) in request.configs {
        let config = SolanaApiPermissionConfig {
            endpoint: endpoint.clone(),
            name: update_request.name,
            category: update_request.category,
            read_policy: update_request.read_policy,
            write_policy: update_request.write_policy,
            enabled: update_request.enabled,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };
        configs_to_update.insert(endpoint, config);
    }

    match services.solana_permission.batch_update_api_configs(configs_to_update).await {
        Ok(_) => {
            info!("Admin {} performed batch update of API configs", auth_user.user_id);
            Ok(Json(ApiResponse::success("批量更新完成".to_string())))
        }
        Err(e) => {
            error!("Failed to batch update API configs: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::error("批量更新失败".to_string()))))
        }
    }
}

/// 根据分类获取API配置
pub async fn get_api_configs_by_category(
    Extension(services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
    Path(category): Path<String>,
) -> Result<Json<ApiResponse<Vec<SolanaApiPermissionConfig>>>, (StatusCode, Json<ApiResponse<()>>)> {
    if !auth_user.is_admin() {
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    match services.solana_permission.get_all_api_configs().await {
        Ok(all_configs) => {
            let filtered_configs: Vec<SolanaApiPermissionConfig> = all_configs.into_values().filter(|config| config.category == category).collect();

            Ok(Json(ApiResponse::success(filtered_configs)))
        }
        Err(e) => {
            error!("Failed to get API configs by category: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<()>::error("获取分类配置失败".to_string()))))
        }
    }
}

/// 获取权限日志
pub async fn get_permission_logs(
    Extension(_services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
    Query(_query): Query<PermissionLogQuery>,
) -> Result<Json<ApiResponse<PermissionLogResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    if !auth_user.is_admin() {
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    // TODO: 实现权限日志查询
    // 这里暂时返回空结果
    let response = PermissionLogResponse {
        logs: vec![],
        total_count: 0,
        page: 1,
        page_size: 20,
    };

    Ok(Json(ApiResponse::success(response)))
}

/// 根据操作者获取日志
pub async fn get_logs_by_operator(
    Extension(_services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
    Path(_operator_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<PermissionLogEntry>>>, (StatusCode, Json<ApiResponse<()>>)> {
    if !auth_user.is_admin() {
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    // TODO: 实现根据操作者查询日志
    // 这里暂时返回空结果
    Ok(Json(ApiResponse::success(vec![])))
}

/// 根据目标获取日志
pub async fn get_logs_by_target(
    Extension(_services): Extension<Services>,
    Extension(auth_user): Extension<AuthUser>,
    Path((_target_type, _target_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Vec<PermissionLogEntry>>>, (StatusCode, Json<ApiResponse<()>>)> {
    if !auth_user.is_admin() {
        return Err((StatusCode::FORBIDDEN, Json(ApiResponse::<()>::error("需要管理员权限".to_string()))));
    }

    // TODO: 实现根据目标查询日志
    // 这里暂时返回空结果
    Ok(Json(ApiResponse::success(vec![])))
}
