use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    BadRequest { code: &'static str, message: String },
    Unauthorized { code: &'static str, message: String },
    NotFound { code: &'static str, message: String },
    Upstream { code: &'static str, message: String },
    Config { code: &'static str, message: String },
    Unsupported { code: &'static str, message: String },
    Internal { code: &'static str, message: String },
}

impl AppError {
    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::BadRequest {
            code,
            message: message.into(),
        }
    }

    pub fn unauthorized(code: &'static str, message: impl Into<String>) -> Self {
        Self::Unauthorized {
            code,
            message: message.into(),
        }
    }

    pub fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self::NotFound {
            code,
            message: message.into(),
        }
    }

    pub fn upstream(code: &'static str, message: impl Into<String>) -> Self {
        Self::Upstream {
            code,
            message: message.into(),
        }
    }

    pub fn config(code: &'static str, message: impl Into<String>) -> Self {
        Self::Config {
            code,
            message: message.into(),
        }
    }

    pub fn unsupported(code: &'static str, message: impl Into<String>) -> Self {
        Self::Unsupported {
            code,
            message: message.into(),
        }
    }

    pub fn internal(code: &'static str, message: impl Into<String>) -> Self {
        Self::Internal {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::BadRequest { code, .. }
            | Self::Unauthorized { code, .. }
            | Self::NotFound { code, .. }
            | Self::Upstream { code, .. }
            | Self::Config { code, .. }
            | Self::Unsupported { code, .. }
            | Self::Internal { code, .. } => code,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::BadRequest { message, .. }
            | Self::Unauthorized { message, .. }
            | Self::NotFound { message, .. }
            | Self::Upstream { message, .. }
            | Self::Config { message, .. }
            | Self::Unsupported { message, .. }
            | Self::Internal { message, .. } => message,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}

impl Error for AppError {}

pub fn map_baidu_errno(errno: i32, context: &'static str) -> AppError {
    match errno {
        0 => AppError::internal(
            "baidu_success_as_error",
            format!("{context}: unexpected success errno"),
        ),
        -9 | -12 => AppError::bad_request("share_password_invalid", "提取码错误或分享验证失败"),
        -7 | 105 | 110 => {
            AppError::not_found("share_unavailable", "分享链接不存在、已失效或已过期")
        }
        -20 => AppError::bad_request("remote_path_missing", "百度网盘保存路径不存在"),
        2 => AppError::unauthorized("baidu_auth_failed", "百度账号登录态失效或权限不足"),
        31045 => AppError::unauthorized("baidu_access_token_invalid", "access_token 无效或已过期"),
        31326 => AppError::upstream(
            "baidu_hotlink_protection",
            "下载请求命中防盗链，请检查 User-Agent",
        ),
        31360 => AppError::upstream("baidu_dlink_expired", "百度 dlink 已过期"),
        31362 => AppError::upstream("baidu_dlink_signature_invalid", "百度 dlink 签名无效"),
        other => AppError::upstream(
            "baidu_errno",
            format!("{context}: 百度接口返回 errno={other}"),
        ),
    }
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        AppError::internal("internal_error", value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_baidu_errno() {
        assert_eq!(
            map_baidu_errno(-9, "verify").code(),
            "share_password_invalid"
        );
        assert_eq!(map_baidu_errno(110, "list").code(), "share_unavailable");
        assert_eq!(
            map_baidu_errno(31326, "download").code(),
            "baidu_hotlink_protection"
        );
    }
}
