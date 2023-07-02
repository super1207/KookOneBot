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
                let rst = kb2.post_to_client(uri,&json_str).await;
                if rst.is_err() {
                    println!("发送心跳事件到HTTP:`{uri}`失败");
                }
            }
        }
    }
}