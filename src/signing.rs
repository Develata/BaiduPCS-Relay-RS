use crate::error::AppError;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedDownload {
    pub fsid: u64,
    pub filename: String,
    pub expires: u64,
    pub sign: String,
    pub url: String,
}

pub fn generate_signed_download(
    secret: &str,
    fsid: u64,
    filename: &str,
    ttl_secs: u64,
) -> Result<SignedDownload, AppError> {
    if secret.is_empty() || secret == "change-me-sign" {
        return Err(AppError::config(
            "weak_sign_secret",
            "WEB_SIGN_SECRET/sign_secret 不能为空或使用默认值",
        ));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let expires = now + ttl_secs;
    let sign = sign_download(secret, fsid, filename, expires);
    let url = format!(
        "/d/download?fsid={}&expires={}&filename={}&sign={}",
        fsid,
        expires,
        urlencoding::encode(filename),
        sign
    );

    Ok(SignedDownload {
        fsid,
        filename: filename.to_string(),
        expires,
        sign,
        url,
    })
}

pub fn verify_signed_download(
    secret: &str,
    fsid: u64,
    filename: &str,
    expires: u64,
    sign: &str,
    now: u64,
) -> Result<(), AppError> {
    if now > expires {
        return Err(AppError::unauthorized(
            "signed_link_expired",
            "下载链接已过期",
        ));
    }

    let expected = sign_download(secret, fsid, filename, expires);
    if !constant_time_eq(expected.as_bytes(), sign.as_bytes()) {
        return Err(AppError::unauthorized(
            "signed_link_invalid",
            "下载链接签名无效",
        ));
    }

    Ok(())
}

fn sign_download(secret: &str, fsid: u64, filename: &str, expires: u64) -> String {
    let payload = format!("{fsid}:{expires}:{filename}");
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts keys of any size");
    mac.update(payload.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_valid_signature() {
        let item = generate_signed_download("secret", 42, "a.txt", 60).unwrap();
        verify_signed_download(
            "secret",
            item.fsid,
            &item.filename,
            item.expires,
            &item.sign,
            item.expires - 1,
        )
        .unwrap();
    }

    #[test]
    fn rejects_tampered_filename() {
        let item = generate_signed_download("secret", 42, "a.txt", 60).unwrap();
        let err = verify_signed_download(
            "secret",
            item.fsid,
            "b.txt",
            item.expires,
            &item.sign,
            item.expires - 1,
        )
        .unwrap_err();
        assert_eq!(err.code(), "signed_link_invalid");
    }

    #[test]
    fn rejects_expired_signature() {
        let item = generate_signed_download("secret", 42, "a.txt", 60).unwrap();
        let err = verify_signed_download(
            "secret",
            item.fsid,
            &item.filename,
            item.expires,
            &item.sign,
            item.expires + 1,
        )
        .unwrap_err();
        assert_eq!(err.code(), "signed_link_expired");
    }
}
