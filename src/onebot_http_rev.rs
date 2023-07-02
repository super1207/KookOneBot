use crate::{G_REVERSE_URL, kook_onebot::KookOnebot};

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
                let rst = KookOnebot::post_to_client(uri,&json_str,kb2.self_id).await;
                if rst.is_err() {
                    log::error!("发送心跳事件到HTTP:`{uri}`失败");
                }
            }
        }
    }
}