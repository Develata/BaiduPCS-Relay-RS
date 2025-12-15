use axum::{
    extract::{Form, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use once_cell::sync::Lazy;
use reqwest::Client;
use serde::Deserialize;
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;
use regex::Regex;
use anyhow::{anyhow, Context};

// 百度 Cookies (建议定期更新)
const COOKIE_BDUSS: &str = "你的BDUSS";
const COOKIE_STOKEN: &str = "你的STOKEN"; 

const SAVE_PATH: &str = "/我的资源";

// 伪装 UA 池
const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36";
const NETDISK_UA: &str = "netdisk;7.0.3.2;PC;PC-Windows;10.0.19041;WindowsBaiduYunGuanJia";
const IPHONE_UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 14_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.0.3 Mobile/15E148 Safari/604.1";

// 官方 API 配置
const APP_KEY: &str = "YOUR_APP_KEY";
const SECRET_KEY: &str = "YOUR_SECRET_KEY";
const INITIAL_REFRESH_TOKEN: &str = "YOUR_REFRESH_TOKEN";
// ------------------

// 官方客户端 UA (用于欺骗风控)
const NETDISK_UA: &str = "netdisk;7.0.3.2;PC;PC-Windows;10.0.19041;WindowsBaiduYunGuanJia";

// ------------------

static LINK_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(https?://pan\.baidu\.com/s/1[a-zA-Z0-9_-]+)").unwrap());
static SURL_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"surl=([a-zA-Z0-9_-]+)").unwrap());
static CODE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"提取码\s*[:：]?\s*([0-9a-zA-Z]{4})").unwrap());
static YUN_DATA_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"yunData\.setData\((\{.*?\})\);").unwrap());

struct AppState {
    client: Client,
    token_manager: RwLock<TokenManager>,
}

struct TokenManager {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
}

#[tokio::main]
async fn main() {
    let client = Client::builder()
        .cookie_store(true)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();

    let shared_state = Arc::new(AppState {
        client,
        token_manager: RwLock::new(TokenManager {
            access_token: String::new(),
            refresh_token: INITIAL_REFRESH_TOKEN.to_string(),
            expires_at: 0,
        }),
    });

    let app = Router::new()
        .route("/", get(index))
        .route("/transfer", post(transfer))
        .route("/download", get(download))
        .with_state(shared_state);

    println!("🚀 Rust 验证码增强版启动: http://{}", SERVER_PORT);
    let listener = tokio::net::TcpListener::bind(SERVER_PORT).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// --- Handlers ---

async fn index() -> Html<String> {
    let html = format!(r#"
<!DOCTYPE html>
<html>
<head>
    <meta name="viewport" content="width=device-width,initial-scale=1">
    <title>极速转存</title>
    <style>
        body{{font-family:-apple-system,BlinkMacSystemFont,sans-serif;max-width:600px;margin:0 auto;padding:20px;background:#f5f5f7}}
        .box{{background:#fff;padding:20px;border-radius:12px;box-shadow:0 4px 6px rgba(0,0,0,0.1)}}
        input{{width:100%;padding:12px;margin:8px 0;border:1px solid #ddd;border-radius:8px;box-sizing:border-box}}
        button{{width:100%;background:#007AFF;color:#fff;padding:12px;border:none;border-radius:8px;font-size:16px;font-weight:600;cursor:pointer}}
    </style>
</head>
<body>
    <div class="box">
        <h2 style="text-align:center;margin-top:0">☁️ 极速转存</h2>
        <form action="/transfer" method="post">
            <input type="password" name="token" placeholder="访问密码" required>
            <input type="text" name="link" placeholder="分享链接 (含提取码)" required autofocus>
            <button>立即转存</button>
        </form>
    </div>
</body>
</html>
"#);
    Html(html)
}

async fn transfer(
    State(state): State<Arc<AppState>>,
    Form(form): Form<TransferForm>,
) -> impl IntoResponse {
    if form.token != AUTH_TOKEN {
        return Html("<h1>❌ 密码错误</h1>".to_string()).into_response();
    }

    let (share_url, code) = match parse_link_and_code(&form.link) {
        Some(res) => res,
        None => return Html("<h1>⚠️ 无效的百度网盘链接</h1>".to_string()).into_response(),
    };

    // 调用核心逻辑，传入用户可能提交的验证码
    let result = do_native_transfer(
        &state.client, 
        &share_url, 
        &code, 
        SAVE_PATH, 
        form.vcode, 
        form.vcode_str
    ).await;

    match result {
        Ok(_) => {
            Html(format!(r#"
            <meta name="viewport" content="width=device-width,initial-scale=1">
            <div style="font-family:sans-serif;text-align:center;padding:40px">
                <h2 style="color:#34C759">✅ 转存成功</h2>
                <p style="color:#666">文件已保存至: {}</p>
                <br><a href="/" style="display:inline-block;padding:10px 20px;background:#007AFF;color:white;border-radius:8px;text-decoration:none">返回首页</a>
            </div>"#, SAVE_PATH)).into_response()
        },
        Err(e) => {
            // 🔥 拦截特定错误：需要验证码
            if let Some(captcha_err) = e.downcast_ref::<CaptchaRequiredError>() {
                // 返回验证码输入页面
                let img_url = format!("https://wappass.baidu.com/cgi-bin/genimage?{}", captcha_err.vcode_str);
                return Html(format!(r#"
                <!DOCTYPE html>
                <html>
                <head><meta name="viewport" content="width=device-width,initial-scale=1">
                <style>body{{font-family:sans-serif;padding:20px;max-width:600px;margin:0 auto;background:#f5f5f7}}.box{{background:#fff;padding:20px;border-radius:12px;text-align:center}}input{{width:100%;padding:12px;margin:10px 0;border:1px solid #ddd;border-radius:8px}}button{{width:100%;padding:12px;background:#FF9500;color:#fff;border:none;border-radius:8px;font-size:16px}}</style></head>
                <body>
                    <div class="box">
                        <h2 style="color:#FF9500">🛡️ 需要人机验证</h2>
                        <img src="{}" style="display:block;margin:0 auto;border-radius:4px;border:1px solid #eee">
                        <form action="/transfer" method="post">
                            <input type="hidden" name="token" value="{}">
                            <input type="hidden" name="link" value="{}">
                            <input type="hidden" name="vcode_str" value="{}">
                            <input type="text" name="vcode" placeholder="输入上图字符" required autofocus autocomplete="off">
                            <button>提交验证</button>
                        </form>
                    </div>
                </body></html>
                "#, img_url, form.token, form.link, captcha_err.vcode_str)).into_response();
            }

            // 其他错误
            Html(format!(r#"<h2 style="color:#FF3B30;text-align:center">❌ 转存失败</h2><p style="text-align:center">{}</p><center><a href="/">返回</a></center>"#, e)).into_response()
        }
    }
}

async fn download(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DownloadParams>,
) -> impl IntoResponse {
    if let Err(e) = refresh_token_logic(&state).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("Token Error: {}", e)).into_response();
    }

    let access_token = {
        let guard = state.token_manager.read().await;
        guard.access_token.clone()
    };
    
    let url = format!("https://pan.baidu.com/rest/2.0/xpan/multimedia?method=filemetas&fsids=[{}]&dlink=1&access_token={}", 
        params.fsid, access_token);

    let resp = match state.client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    };

    let dlink_res: DlinkResponse = match resp.json().await {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    if let Some(list) = dlink_res.list {
        if !list.is_empty() {
            let final_url = format!("{}&access_token={}", list[0].dlink, access_token);
            return Redirect::to(&final_url).into_response();
        }
    }
    
    (StatusCode::NOT_FOUND, "File Not Found".to_string()).into_response()
}

// --- 🔥 核心业务逻辑 ---

// 自定义错误类型：验证码需求
#[derive(Debug)]
struct CaptchaRequiredError {
    vcode_str: String,
}
impl std::fmt::Display for CaptchaRequiredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Captcha Required")
    }
}
impl std::error::Error for CaptchaRequiredError {}

async fn do_native_transfer(
    client: &Client, 
    share_url: &str, 
    password: &str, 
    remote_dir: &str,
    vcode_input: Option<String>,
    vcode_str: Option<String>
) -> anyhow::Result<()> {
    let mut surl = String::new();
    if let Some(idx) = share_url.find("/s/1") {
        surl = share_url[idx+4..].to_string(); 
    } else if let Some(caps) = SURL_REGEX.captures(share_url) {
        surl = caps[1].to_string(); 
    }
    if surl.is_empty() { return Err(anyhow!("无法解析 Surl")); }

    // 1. 验证提取码 (如果存在)
    if !password.is_empty() {
        let verify_url = format!("https://pan.baidu.com/share/verify?channel=chunlei&clienttype=0&web=1&t={}&surl={}", now_ts(), surl);
        let params = [("pwd", password), ("vcode", ""), ("vcode_str", "")];
        
        let resp = client.post(&verify_url)
            .header("Referer", "https://pan.baidu.com/disk/home") 
            .form(&params).send().await?;
        
        let verify_res: BaiduErrno = resp.json().await?;
        if verify_res.errno != 0 {
            return Err(anyhow!("提取码错误 (errno: {})", verify_res.errno));
        }
        
        // 关键延时
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    // 2. 获取页面参数
    let page_resp = client.get(share_url)
        .header("Cookie", format!("BDUSS={}; STOKEN={};", COOKIE_BDUSS, COOKIE_STOKEN))
        .send().await?;
    let html = page_resp.text().await?;
    
    // 🔥🔥 修改点：增强错误诊断 🔥🔥
    let caps = match YUN_DATA_REGEX.captures(&html) {
        Some(c) => c,
        None => {
            // 解析失败，分析 HTML 内容找原因
            let title_regex = Regex::new(r"<title>(.*?)</title>").unwrap();
            let page_title = title_regex.captures(&html)
                .map(|c| c[1].to_string())
                .unwrap_or_else(|| "未知标题".to_string());

            // 1. 链接失效
            if html.contains("链接不存在") || html.contains("此链接分享内容可能因为") || html.contains("啊哦，你来晚了") || page_title.contains("百度网盘-链接不存在") {
                return Err(anyhow!("❌ 转存失败: 分享链接已失效或被取消"));
            }
            // 2. Cookie 失效 (跳转到了登录页)
            if html.contains("百度帐号登录") || page_title.contains("百度网盘-登录") {
                return Err(anyhow!("❌ 转存失败: Cookie (BDUSS) 已失效，请重新获取"));
            }
            // 3. 页面级验证码 (访问页面本身就需要验证码)
            if html.contains("验证码") || html.contains("verify") {
                return Err(anyhow!("❌ 转存失败: 访问分享页触发了验证码 (IP被风控)，请稍后重试"));
            }
            
            // 4. 其他未知原因 (打印标题方便调试)
            return Err(anyhow!("❌ 页面解析失败 (页面标题: {}) - 可能是网络问题或百度改版", page_title));
        }
    };

    let yun_data: YunData = serde_json::from_str(&caps[1]).context("YunData 解析失败")?;

    if yun_data.file_list.is_empty() { return Err(anyhow!("分享文件列表为空")); }

    let fs_ids: Vec<u64> = yun_data.file_list.iter().map(|f| f.fs_id).collect();
    let fs_ids_json = serde_json::to_string(&fs_ids)?;

    // 3. 执行转存
    let transfer_url = format!(
        "https://pan.baidu.com/share/transfer?shareid={}&from={}&ondup=newcopy&async=1&channel=chunlei&clienttype=0&web=1&bdstoken={}",
        yun_data.shareid, yun_data.uk, yun_data.bdstoken
    );

    let mut params = vec![
        ("fsidlist", fs_ids_json), 
        ("path", remote_dir.to_string())
    ];

    if let (Some(vc), Some(vcs)) = (vcode_input, vcode_str) {
        params.push(("vcode", vc));
        params.push(("vcode_str", vcs));
    }

    let resp = client.post(&transfer_url)
        .header("Cookie", format!("BDUSS={}; STOKEN={};", COOKIE_BDUSS, COOKIE_STOKEN))
        .header("Referer", "https://pan.baidu.com/disk/home")
        .header("User-Agent", NETDISK_UA)
        .form(&params)
        .send().await?;

    let res_text = resp.text().await?;
    let res: TransferResult = serde_json::from_str(&res_text).unwrap_or(TransferResult { 
        errno: -1, 
        vcode: None, 
        _img: None,
        errmsg: None,
    });

    match res.errno {
        0 => Ok(()),
        12 => Ok(()),
        -19 | -62 => {
            if let Some(vcode_str) = res.vcode {
                Err(anyhow::Error::new(CaptchaRequiredError { vcode_str }))
            } else {
                Err(anyhow!("触发验证码但未获取到 Session ID"))
            }
        },
        _ => Err(anyhow!("转存失败 (errno: {} msg: {})", res.errno, res.errmsg.unwrap_or_default()))
    }
}
// --- 辅助逻辑 ---

fn parse_link_and_code(raw: &str) -> Option<(String, String)> {
    let link = LINK_REGEX.captures(raw).map(|c| c[1].to_string())?;
    let code = CODE_REGEX.captures(raw).map(|c| c[1].to_string()).unwrap_or_default();
    Some((link, code))
}

async fn refresh_token_logic(state: &Arc<AppState>) -> anyhow::Result<()> {
    {
        let tm = state.token_manager.read().await;
        if !tm.access_token.is_empty() && now_ts() < tm.expires_at - 600 { return Ok(()); }
    }
    
    let mut tm = state.token_manager.write().await;
    if !tm.access_token.is_empty() && now_ts() < tm.expires_at - 600 { return Ok(()); }

    let url = format!("https://openapi.baidu.com/oauth/2.0/token?grant_type=refresh_token&refresh_token={}&client_id={}&client_secret={}",
        tm.refresh_token, APP_KEY, SECRET_KEY);
    
    let resp = state.client.get(&url).send().await?;
    let token_res: TokenResponse = resp.json().await?;
    
    if let Some(at) = token_res.access_token {
        tm.access_token = at;
        tm.expires_at = now_ts() + 2592000;
        if let Some(rt) = token_res.refresh_token { tm.refresh_token = rt; }
        Ok(())
    } else {
        Err(anyhow!("Token刷新失败"))
    }
}

fn now_ts() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

// --- 结构体定义 (更新版) ---

#[derive(Deserialize)] 
struct TransferForm { 
    token: String, 
    link: String,
    // 可选参数：用于验证码重试
    vcode: Option<String>,     // 用户输入的4位字符
    vcode_str: Option<String>, // 百度下发的 Session ID
}

#[derive(Deserialize)] struct DownloadParams { fsid: u64 }
#[derive(Deserialize)] struct BaiduErrno { errno: i32 }

#[derive(Deserialize, Debug)] 
struct TransferResult { 
    errno: i32,
    // 百度返回的验证码字段
    vcode: Option<String>, // 这里实际上返回的是 vcode_str (hash)
    #[serde(alias = "img")]
    _img: Option<String>, // 加了下划线
    #[serde(alias = "show_msg")] // 错误信息
    errmsg: Option<String>,
}

#[derive(Deserialize)]
struct YunData {
    #[serde(deserialize_with = "de_str_or_num")] shareid: String,
    #[serde(deserialize_with = "de_str_or_num")] uk: String,
    bdstoken: String,
    file_list: Vec<YunFile>,
}
#[derive(Deserialize)] struct YunFile { fs_id: u64 }

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
}
#[derive(Deserialize)] struct DlinkResponse { list: Option<Vec<DlinkItem>> }
#[derive(Deserialize)] struct DlinkItem { dlink: String }

fn de_str_or_num<'de, D>(deserializer: D) -> Result<String, D::Error>
where D: serde::Deserializer<'de> {
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        _ => Err(serde::de::Error::custom("not string or number")),
    }
}