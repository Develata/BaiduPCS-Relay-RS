//! 端到端测试：列出目录 -> 选取文件 -> 调用 /api/zip 打包下载
//!
//! 用法示例：
//! - 容器内：
//!   cargo run --bin test-zip-download -- --dir "/我的资源" --count 5 --out /tmp/test.zip
//!
//! 可选参数：
//!   --config <path>    配置文件路径（默认 config.toml）
//!   --base <url>       Web 服务地址（默认 http://127.0.0.1:5200）
//!   --token <token>    Web 访问密码（默认读取 config.web.access_token）
//!   --dir <path>       网盘目录（默认读取 config.baidu.save_path）
//!   --count <n>        选取前 n 个文件（默认 5）
//!   --archive <name>   ZIP 名称（默认 "test.zip"）
//!   --out <path>       输出文件路径（默认 ./test.zip）

use anyhow::{anyhow, Context, Result};
use baidu_direct_link::{config::Config, AppState};
use futures_util::StreamExt;
use serde::Deserialize;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

#[derive(Debug)]
struct Args {
    config_path: String,
    base_url: String,
    token: Option<String>,
    dir: Option<String>,
    count: usize,
    archive_name: String,
    out_path: String,
}

fn parse_args() -> Result<Args> {
    let mut args = std::env::args().skip(1);

    let mut out = Args {
        config_path: "config.toml".to_string(),
        base_url: "http://127.0.0.1:5200".to_string(),
        token: None,
        dir: None,
        count: 5,
        archive_name: "test.zip".to_string(),
        out_path: "./test.zip".to_string(),
    };

    while let Some(a) = args.next() {
        match a.as_str() {
            "--config" => {
                out.config_path = args.next().ok_or_else(|| anyhow!("--config 缺少参数"))?
            }
            "--base" => out.base_url = args.next().ok_or_else(|| anyhow!("--base 缺少参数"))?,
            "--token" => out.token = Some(args.next().ok_or_else(|| anyhow!("--token 缺少参数"))?),
            "--dir" => out.dir = Some(args.next().ok_or_else(|| anyhow!("--dir 缺少参数"))?),
            "--count" => {
                let v = args.next().ok_or_else(|| anyhow!("--count 缺少参数"))?;
                out.count = v.parse::<usize>().context("--count 需要是整数")?;
            }
            "--archive" => {
                out.archive_name = args.next().ok_or_else(|| anyhow!("--archive 缺少参数"))?
            }
            "--out" => out.out_path = args.next().ok_or_else(|| anyhow!("--out 缺少参数"))?,
            "-h" | "--help" => {
                println!("{}", include_str!("../../README.md"));
                return Err(anyhow!("已显示帮助（请忽略该错误退出）"));
            }
            other => return Err(anyhow!("未知参数: {other}")),
        }
    }

    Ok(out)
}

#[derive(Deserialize)]
struct ListResult {
    errno: i32,
    #[serde(default)]
    list: Vec<ListEntry>,
}

#[derive(Deserialize)]
struct ListEntry {
    #[serde(rename = "fs_id")]
    fsid: u64,
    #[serde(default)]
    server_filename: String,
    #[serde(default)]
    isdir: i32,
}

#[derive(serde::Serialize)]
struct ZipRequest {
    fsids: Vec<u64>,
    archive_name: String,
    token: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 日志简单输出（避免重复 init 导致 panic）
    let _ = tracing_subscriber::fmt()
        .with_env_filter("baidu_direct_link=info")
        .try_init();

    let args = parse_args()?;

    println!("🧪 ZIP 打包下载测试");
    println!("- config: {}", args.config_path);
    println!("- base:   {}", args.base_url);
    println!("- archive:{}", args.archive_name);
    println!("- out:    {}", args.out_path);

    let config = Config::load(&args.config_path)?;
    let token = args
        .token
        .clone()
        .unwrap_or_else(|| config.web.access_token.clone());
    let dir = args
        .dir
        .clone()
        .unwrap_or_else(|| config.baidu.save_path.clone());

    println!("- dir:    {}", dir);
    println!("- count:  {}", args.count);

    let state = Arc::new(AppState::new(config)?);

    // 1) 列目录（优先选择文件夹进行打包测试；若无文件夹则退化为选取文件）
    let list_url = format!(
        "https://pan.baidu.com/api/list?dir={}&num=1000&order=time&desc=1",
        urlencoding::encode(&dir)
    );

    let resp = state
        .client
        .get(&list_url)
        .header(
            "User-Agent",
            baidu_direct_link::config::Config::browser_ua(),
        )
        .send()
        .await
        .context("请求目录列表失败")?;

    let text = resp.text().await.context("读取目录列表响应失败")?;
    let result: ListResult = serde_json::from_str(&text).map_err(|e| {
        anyhow!(
            "解析目录列表失败: {e}, body={}",
            &text[..text.len().min(300)]
        )
    })?;

    if result.errno != 0 {
        return Err(anyhow!(
            "目录列表 errno={}（可能 Cookie 失效或路径不存在）",
            result.errno
        ));
    }

    let mut folders: Vec<ListEntry> = Vec::new();
    let mut files: Vec<ListEntry> = Vec::new();
    for e in result.list {
        if e.isdir == 1 {
            folders.push(e);
        } else {
            files.push(e);
        }
    }

    let fsids: Vec<u64> = if let Some(folder) = folders.first() {
        println!(
            "✅ 检测到文件夹，优先测试文件夹打包：{} (fsid={})",
            folder.server_filename, folder.fsid
        );
        println!("   将请求 /api/zip 传入该文件夹 fsid，由后端递归展开并打包 ZIP");
        vec![folder.fsid]
    } else {
        if files.is_empty() {
            return Err(anyhow!("目录为空（没有文件也没有文件夹）"));
        }

        let take_n = args.count.min(files.len());
        let chosen = &files[..take_n];

        println!("✅ 未检测到文件夹，退化为打包文件：选取前 {} 个", take_n);
        for (i, f) in chosen.iter().enumerate() {
            println!("  {}. {} (fsid={})", i + 1, f.server_filename, f.fsid);
        }

        chosen.iter().map(|e| e.fsid).collect()
    };

    // 2) 调 /api/zip 打包下载
    let zip_api = format!("{}/api/zip", args.base_url.trim_end_matches('/'));
    let body = ZipRequest {
        fsids,
        archive_name: args.archive_name.clone(),
        token,
    };

    println!("\n🌐 POST {}", zip_api);

    let zip_resp = reqwest::Client::new()
        .post(&zip_api)
        .json(&body)
        .send()
        .await
        .context("请求 /api/zip 失败")?;

    let status = zip_resp.status();
    let ct = zip_resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !status.is_success() {
        let err_text = zip_resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "/api/zip 返回失败 status={}, body={}",
            status,
            err_text
        ));
    }

    if !ct.contains("application/zip") {
        eprintln!("⚠️  Content-Type 看起来不是 application/zip: {}", ct);
    }

    let mut out_file = tokio::fs::File::create(&args.out_path)
        .await
        .context("创建输出文件失败")?;

    let mut total: u64 = 0;
    let mut s = zip_resp.bytes_stream();
    while let Some(chunk) = s.next().await {
        let chunk = chunk.context("读取 ZIP 流 chunk 失败")?;
        out_file
            .write_all(&chunk)
            .await
            .context("写入 ZIP 文件失败")?;
        total += chunk.len() as u64;

        if total % (32 * 1024 * 1024) < chunk.len() as u64 {
            println!("... 已写入 {} MB", total / (1024 * 1024));
        }
    }

    out_file.flush().await.ok();

    println!(
        "\n✅ ZIP 流式下载完成: {} bytes -> {}",
        total, args.out_path
    );
    Ok(())
}
