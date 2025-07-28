use axum::{
    extract::{ConnectInfo, Request},
    middleware::Next,
    response::Response,
};
use std::net::SocketAddr;
use std::time::Instant;
use tracing::info;

/// 请求日志中间件
/// 记录每个HTTP请求的IP地址、方法、路径和响应时间
pub async fn request_logger(ConnectInfo(addr): ConnectInfo<SocketAddr>, request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = Instant::now();

    // 提取客户端IP地址
    let client_ip = addr.ip();

    // 记录请求开始
    info!(
        "🌐 请求开始 - IP: {} | {} {} | User-Agent: {:?}",
        client_ip,
        method,
        uri,
        request.headers().get("user-agent").map(|h| h.to_str().unwrap_or("Unknown"))
    );

    // 处理请求
    let response = next.run(request).await;

    // 计算处理时间
    let duration = start.elapsed();
    let status = response.status();

    // 记录请求完成
    info!(
        "✅ 请求完成 - IP: {} | {} {} | 状态: {} | 耗时: {:.2}ms",
        client_ip,
        method,
        uri,
        status.as_u16(),
        duration.as_secs_f64() * 1000.0
    );

    response
}

/// 简化版本的IP记录中间件，只记录IP和基本信息
pub async fn simple_ip_logger(ConnectInfo(addr): ConnectInfo<SocketAddr>, request: Request, next: Next) -> Response {
    let method = request.method();
    let path = request.uri().path();
    let client_ip = addr.ip();

    // 记录请求IP
    info!("📍 API请求 - IP: {} | {} {}", client_ip, method, path);

    // 处理请求
    next.run(request).await
}
