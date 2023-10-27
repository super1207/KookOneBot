use std::str::FromStr;

use hyper::http::{HeaderName, HeaderValue};

use crate::{G_REVERSE_URL, kook_onebot::KookOnebot, G_SECERT};

use hmac::{Hmac, Mac};
use sha1::Sha1;

pub async fn post_to_client(url:&str,json_str:&str,self_id:u64) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let secert = G_SECERT.read().await.clone();
    let uri = reqwest::Url::from_str(url)?;
    let client = reqwest::Client::builder().danger_accept_invalid_certs(true).no_proxy().build()?;
    let mut req = client.post(uri).body(reqwest::Body::from(json_str.to_owned())).build()?;
    if secert != "" {
        type HmacSha1 = Hmac<Sha1>;
        let secert = G_SECERT.read().await.clone();
        let secert_str = secert.as_bytes();
        let mut mac = HmacSha1::new_from_slice(&secert_str).expect("HMAC can take key of any size");
        mac.update(json_str.as_bytes());
        let result = mac.finalize();
        let code_bytes = result.into_bytes();
        let sha1_str = hex::encode(code_bytes);
        req.headers_mut().append(HeaderName::from_str("X-Signature")?, HeaderValue::from_str(&format!("sha1={sha1_str}"))?);
    }
    req.headers_mut().append(HeaderName::from_str("Content-type")?, HeaderValue::from_str("application/json")?);
    req.headers_mut().append(HeaderName::from_str("X-Self-ID")?, HeaderValue::from_str(&self_id.to_string())?);
    let res= client.execute(req).await?;
    let res_code = res.status();
    let mut res_json = serde_json::Value::Null;
    if res_code != reqwest::StatusCode::NO_CONTENT {
        let res_content = res.bytes().await?;
        if res_content.len() != 0 {
            res_json = serde_json::from_slice(&res_content)?;
        }
    }
    Ok(res_json)
}


pub async fn deal_heartbeat(kb2:KookOnebot) -> ! {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        {
            let json_str = kb2.get_heartbeat_event().await.unwrap();
            let lk = G_REVERSE_URL.read().await;
            for uri in &*lk {
                if !uri.starts_with("http") {
                    continue;
                }
                let rst = post_to_client(uri,&json_str,kb2.self_id).await;
                if rst.is_err() {
                    log::error!("发送心跳事件到HTTP:`{uri}`失败");
                }
            }
        }
    }
}