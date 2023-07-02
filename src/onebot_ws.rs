use std::sync::{Arc, atomic::AtomicI64};

use futures_util::{SinkExt, StreamExt};

use crate::{G_KOOK_TOKEN, G_SELF_ID, G_ONEBOT_RX};

// 正向ws
async fn deal_ws(uid:&str,
    mut write_half: futures_util::stream::SplitSink<hyper_tungstenite::WebSocketStream<hyper::upgrade::Upgraded>, hyper_tungstenite::tungstenite::Message>,
    mut read_half: futures_util::stream::SplitStream<hyper_tungstenite::WebSocketStream<hyper::upgrade::Upgraded>>
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    
    let kb = crate::kook_onebot::KookOnebot {
        token: G_KOOK_TOKEN.read().await.to_owned(),
        self_id: G_SELF_ID.read().await.to_owned(),
        sn: Arc::new(AtomicI64::new(0)),
    };

    // 获得升级后的ws流
    let (tx, mut rx) =  tokio::sync::mpsc::channel::<String>(60);
    {
        let mut lk = G_ONEBOT_RX.write().await;
        lk.insert(uid.to_owned(), (tx.clone(),"".to_owned()));
    }
    let _guard = scopeguard::guard(uid.to_owned(), |uid: String| {
        tokio::spawn(async move {
            let mut lk = G_ONEBOT_RX.write().await;
            lk.remove(&uid);
        });
    });
    

    // 向onebot客户端发送生命周期包
    let life_event = kb.get_lifecycle_event().await?;
    write_half.send(hyper_tungstenite::tungstenite::Message::Text(life_event)).await?;

    let heartbeat = kb.get_heartbeat_event().await?;
    let tx_copy = tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            let ret = tx_copy.send(heartbeat.clone()).await;
            if ret.is_err() {
                log::error!("ONEBOT_WS心跳包发送出错:{}",ret.err().unwrap());
                break;
            }
        }
    });

    tokio::spawn(async move {
        // 将收到的事件发送到onebot客户端
        while let Some(msg) = rx.recv().await {
            let ret = write_half.send(hyper_tungstenite::tungstenite::Message::Text(msg)).await;
            if ret.is_err() {
                log::error!("ONEBOT_WS数据发送出错:{}",ret.err().unwrap());
                break;
            }
        }
    });

    // 接收来自onebot客户端的调用
    while let Some(msg_t) = read_half.next().await {
        let msg = msg_t?;
        if ! msg.is_text() {
            continue;
        }
        let msg_text = msg.to_text()?;
        // 处理onebot的api调用
        let ret = kb.deal_onebot(uid,msg_text).await;
        if ret.is_err() {
            log::error!("ONEBOT_WS动作调用出错:{ret:?}");
        }else {
            // 发回到onebot客户端
            tx.send(ret.unwrap()).await?;
        }
    }
    Ok(())
}


pub async fn deal_onebot_ws(uid:&str,websocket: hyper_tungstenite::HyperWebsocket) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream: hyper_tungstenite::WebSocketStream<hyper::upgrade::Upgraded> = websocket.await?;
    let (write_half, read_half ) = futures_util::StreamExt::split(ws_stream);
    deal_ws(uid,write_half,read_half).await?;
    Ok(())
}

