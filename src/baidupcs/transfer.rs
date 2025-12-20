//! 百度网盘转存功能模块
//!
//! 参考 baidupcs-go 实现

use anyhow::{anyhow, Result};
use serde::Deserialize;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::AppState;
use chrono::Utc;

/// 在目标网盘上创建目录（如果 API 支持）
async fn create_remote_dir(state: &AppState, path: &str, bdstoken: &str) -> Result<bool> {
    info!("🔧 尝试创建远程目录: {}", path);
    let url = format!(
        "https://pan.baidu.com/rest/2.0/xpan/file?method=create&path={}&isdir=1&bdstoken={}",
        urlencoding::encode(path),
        bdstoken
    );

    let resp = state
        .client
        .post(&url)
        .header("User-Agent", Config::browser_ua())
        .header("Referer", "https://pan.baidu.com/")
        .header("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
        .send()
        .await?;

    let text = resp.text().await?;
    debug!("create dir 响应: {}", text);

    #[derive(Deserialize)]
    struct CreateResponse {
        errno: i32,
        #[serde(default)]
        request_id: Option<u64>,
        #[serde(default)]
        err_msg: Option<String>,
    }

    let res: CreateResponse =
        serde_json::from_str(&text).map_err(|e| anyhow!("解析 create 响应失败: {}, body={}", e, text))?;

    if res.errno == 0 {
        info!("✅ 远程目录创建成功: {}", path);
        Ok(true)
    } else {
        warn!("❌ 远程目录创建失败 (errno={}): {:?}", res.errno, res.err_msg);
        Ok(false)
    }
}

#[derive(Debug, Deserialize)]
struct TransferResult {
    #[serde(default)]
    errno: i32,
    #[serde(default)]
    show_msg: String,
    #[serde(default)]
    newno: String,
    #[serde(default)]
    request_id: Option<u64>,
}

/// 验证保存路径是否存在
pub async fn verify_save_path(state: &AppState, path: &str) -> Result<bool> {
    info!("🔍 验证保存路径: {}", path);

    let url = format!(
        "https://pan.baidu.com/api/list?dir={}&num=1&order=name&desc=0",
        urlencoding::encode(path)
    );

    let resp = state
        .client
        .get(&url)
        .header("User-Agent", Config::browser_ua())
        .send()
        .await?;

    let text = resp.text().await?;
    debug!("路径验证响应: {}", text);

    #[derive(Deserialize)]
    struct ApiListResponse {
        errno: i32,
    }

    let result: ApiListResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("路径验证响应解析失败: {}, body={}", e, text))?;
    let errno = result.errno;

    if errno == 0 {
        info!("✅ 保存路径存在");
        Ok(true)
    } else {
        warn!("❌ 保存路径不存在 (errno={})", errno);
        warn!("💡 请在百度网盘中先创建该文件夹: {}", path);
        Ok(false)
    }
}

/// 百度网盘转存 API
///
/// # 参考 baidupcs-go 实现
pub async fn transfer_files(
    state: &AppState,
    shareid: &str,
    uk: &str,
    fs_ids: &[u64],
    bdstoken: &str,
    surl: &str,
) -> Result<()> {
    info!("📦 开始转存 {} 个文件...", fs_ids.len());

    let savepath = &state.config.baidu.save_path; // ← 改成 save_path

    // 先验证保存路径
    if !verify_save_path(state, savepath).await? {
        return Err(anyhow!(
            "保存路径不存在: {}，请先在百度网盘中创建该文件夹",
            savepath
        ));
    }

    // 构建转存 URL
    // ondup参数: newcopy(重命名), overwrite(覆盖), fail(失败)
    let url = format!(
        "https://pan.baidu.com/share/transfer?shareid={}&from={}&ondup=newcopy&channel=chunlei&clienttype=0&web=1&bdstoken={}",
        shareid, uk, bdstoken
    );

    let fsidlist = serde_json::to_string(fs_ids)?;

    let params = [("fsidlist", fsidlist.as_str()), ("path", savepath.as_str())];

    // 详细日志
    info!("📋 转存参数:");
    info!("  └─ URL: {}", url);
    info!("  └─ shareid: {}", shareid);
    info!("  └─ from(uk): {}", uk);
    info!("  └─ fsidlist: {}", fsidlist);
    info!("  └─ 保存路径: {}", savepath);
    info!("  └─ 重复处理: newcopy (自动重命名)");

    // 移除 surl 前缀 '1'（如果存在）
    let surlparam = surl.strip_prefix('1').unwrap_or(surl);
    let referer = format!("https://pan.baidu.com/share/init?surl={}", surlparam);

    info!("  └─ Referer: {}", referer);

    // 先访问 referer 页面，确保 Cookie 正确
    debug!("🌐 预访问 referer 页面...");
    let _ = state
        .client
        .get(&referer)
        .header("User-Agent", Config::browser_ua())
        .send()
        .await;

    // 调用转存 API
    info!("🚀 发送转存请求...");
    let resp = state
        .client
        .post(&url)
        .header("User-Agent", Config::browser_ua())
        .header("Referer", &referer)
        .header("Host", "pan.baidu.com")
        .header("Origin", "https://pan.baidu.com")
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .header("Accept", "application/json, text/javascript, */*; q=0.01")
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .header("X-Requested-With", "XMLHttpRequest")
        .form(&params)
        .send()
        .await?;

    let status = resp.status();
    debug!("📡 HTTP状态码: {}", status);

    let text = resp.text().await?;
    info!("📨 转存响应: {}", text);

    let result: TransferResult =
        serde_json::from_str(&text).map_err(|e| anyhow!("解析响应失败: {}, body: {}", e, text))?;
    // 记录更多响应细节，便于诊断
    debug!("🔍 转存响应详情: errno={}, request_id={:?}, newno='{}', show_msg='{}'", result.errno, result.request_id, result.newno, result.show_msg);

    // 详细的 errno 处理
    match result.errno {
        0 => {
            info!("✅ 转存成功! (errno=0)");
            info!("📂 文件已保存至: {}", savepath);
            Ok(())
        }
        2 => {
            // errno=2 有多种含义，需要详细判断
            warn!("⚠️ errno=2 - 详细诊断:");
            warn!("  └─ show_msg: {}", result.show_msg);
            warn!("  └─ request_id: {:?}", result.request_id);
            warn!("  └─ newno: '{}'", result.newno);

            let msg_lower = result.show_msg.to_lowercase();

            if msg_lower.contains("已经保存过")
                || msg_lower.contains("已存在")
                || msg_lower.contains("重复转存")
                || msg_lower.contains("duplicate")
            {
                // 如果 server 没有返回 newno（为空），说明并未创建新副本，需谨慎处理
                if result.newno.is_empty() {
                    error!("❗ server 返回已存在但未创建 new copy (newno empty). 这通常表示目标位置已有相同文件或转存未实际写入。");
                    error!("  └─ show_msg: {}", result.show_msg);
                    error!("  └─ request_id: {:?}", result.request_id);
                    // 尝试按策略 A：在保存路径下创建带时间戳的子目录并重试一次转存
                    let timestamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
                    let new_dir = format!("{}/copy-{}", savepath.trim_end_matches('/'), timestamp);
                    info!("🔁 尝试创建子目录并重试转存: {}", new_dir);
                    match create_remote_dir(state, &new_dir, bdstoken).await {
                        Ok(created) => {
                            if created {
                                info!("✅ 子目录创建成功，尝试在新目录执行转存...");
                                // 重试转存到 new_dir
                                let retry_params = [("fsidlist", fsidlist.as_str()), ("path", new_dir.as_str())];
                                let retry_resp = state
                                    .client
                                    .post(&url)
                                    .header("User-Agent", Config::browser_ua())
                                    .header("Referer", &referer)
                                    .header("Host", "pan.baidu.com")
                                    .header("Origin", "https://pan.baidu.com")
                                    .header(
                                        "Content-Type",
                                        "application/x-www-form-urlencoded; charset=UTF-8",
                                    )
                                    .header("Accept", "application/json, text/javascript, */*; q=0.01")
                                    .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
                                    .header("X-Requested-With", "XMLHttpRequest")
                                    .form(&retry_params)
                                    .send()
                                    .await?;
                                let retry_text = retry_resp.text().await?;
                                info!("📨 重试转存响应: {}", retry_text);
                                let retry_result: TransferResult = serde_json::from_str(&retry_text)
                                    .map_err(|e| anyhow!("解析重试响应失败: {}, body: {}", e, retry_text))?;
                                if retry_result.errno == 0 || (retry_result.errno == 12) {
                                    info!("✅ 重试转存成功 (errno={})", retry_result.errno);
                                    return Ok(());
                                } else {
                                    error!("❌ 重试转存仍然失败: errno={}, show_msg={}", retry_result.errno, retry_result.show_msg);
                                    return Err(anyhow!("重试转存失败: {}", retry_result.show_msg));
                                }
                            } else {
                                error!("❌ 子目录创建返回失败，无法重试转存");
                                return Err(anyhow!("文件已存在，且无法创建子目录重试: {}", result.show_msg));
                            }
                        }
                        Err(e) => {
                            error!("❌ 创建子目录失败: {}", e);
                            return Err(anyhow!("文件已存在，且创建子目录失败: {} ({})", result.show_msg, e));
                        }
                    }
                } else {
                    info!("📁 文件已存在（已创建副本 newno={}），转存完成", result.newno);
                    info!("💡 提示: {}", result.show_msg);
                    Ok(())
                }
            } else if msg_lower.contains("未登录")
                || msg_lower.contains("需要登录")
                || msg_lower.contains("登陆")
                || msg_lower.contains("验证")
                || msg_lower.contains("login")
            {
                error!("🔐 Cookie 失效或未登录!");
                error!("📝 请检查 config.toml 中的:");
                error!("   1. cookie_bduss (长度应为192字符)");
                error!("   2. cookie_stoken (长度应为32字符)");
                error!("💡 获取方式:");
                error!("   1. 浏览器登录 pan.baidu.com");
                error!("   2. F12 打开开发者工具");
                error!("   3. Application -> Cookies -> BDUSS 和 STOKEN");
                Err(anyhow!("Cookie失效: {}", result.show_msg))
            } else if msg_lower.contains("路径")
                || msg_lower.contains("目录")
                || msg_lower.contains("文件夹")
                || msg_lower.contains("path")
            {
                error!("📂 保存路径问题: {}", result.show_msg);
                error!("📝 当前保存路径: {}", savepath);
                error!("💡 请确保该文件夹在百度网盘中存在");
                Err(anyhow!("路径错误: {}", result.show_msg))
            } else if msg_lower.contains("权限") || msg_lower.contains("permission") {
                error!("🚫 权限不足: {}", result.show_msg);
                error!("💡 可能原因:");
                error!("   1. 分享链接已失效");
                error!("   2. 分享者设置了权限限制");
                Err(anyhow!("权限不足: {}", result.show_msg))
            } else {
                // 未知的 errno=2 错误
                error!("❌ 未知的 errno=2 错误");
                error!("  └─ show_msg: {}", result.show_msg);
                error!("  └─ 完整响应: {}", text);
                error!("💡 建议:");
                error!("   1. 检查 Cookie 是否有效");
                error!("   2. 尝试修改保存路径为 /apps 或 /test");
                error!("   3. 确认分享链接有效");
                Err(anyhow!("转存失败: {}", result.show_msg))
            }
        }
        12 => {
            info!("✅ 转存完成 (errno=12)");
            info!("💡 errno=12 通常表示文件已存在或部分成功");
            Ok(())
        }
        -1 => {
            error!("❌ 转存失败: 文件不存在或已删除");
            Err(anyhow!("文件不存在"))
        }
        -7 => {
            error!("❌ 转存失败: 分享链接无效或已过期");
            Err(anyhow!("分享链接失效"))
        }
        -9 => {
            error!("❌ 转存失败: 提取码错误");
            Err(anyhow!("提取码错误"))
        }
        -20 => {
            error!("❌ 转存失败: 保存路径不存在");
            error!("📝 当前路径: {}", savepath);
            error!("💡 请在百度网盘中创建该文件夹");
            Err(anyhow!("路径不存在: {}", savepath))
        }
        110 => {
            error!("❌ 转存失败: 分享链接已过期");
            Err(anyhow!("分享已过期"))
        }
        _ => {
            error!("❌ 转存失败");
            error!("  └─ errno: {}", result.errno);
            error!("  └─ show_msg: {}", result.show_msg);
            error!("  └─ 完整响应: {}", text);
            Err(anyhow!(
                "转存失败: errno={}, {}",
                result.errno,
                result.show_msg
            ))
        }
    }
}

/// 批量转存（预留接口）
pub async fn do_transfer(
    state: std::sync::Arc<AppState>,
    shareid: String,
    uk: String,
    fsids: Vec<u64>,
    bdstoken: String,
    surl: &str,
    _savepath: &str,
) -> Result<Vec<u64>> {
    transfer_files(state.as_ref(), &shareid, &uk, &fsids, &bdstoken, surl).await?;
    Ok(fsids)
}
