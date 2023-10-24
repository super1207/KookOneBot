use std::{sync::{Arc, atomic::AtomicI64}, str::FromStr};

use futures_util::{StreamExt, SinkExt};
use hyper::http::{HeaderValue, HeaderName};
use tokio::net::TcpStream;

use crate::{G_REVERSE_URL, G_ONEBOT_RX, G_KOOK_TOKEN, G_SELF_ID, G_ACCESS_TOKEN};


// 反向ws
async fn deal_ws2(url:&str,
    mut write_half:futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, tungstenite::Message>,
    mut read_half: futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    
    let kb = crate::kook_onebot::KookOnebot {
        token: G_KOOK_TOKEN.read().await.to_owned(),
        self_id: G_SELF_ID.read().await.to_owned(),
        sn: Arc::new(AtomicI64::new(0)),
    };
    let uid = uuid::Uuid::new_v4().to_string();

    // 获得升级后的ws流
    let (tx, mut rx) =  tokio::sync::mpsc::channel::<String>(60);
    {
        let mut lk = G_ONEBOT_RX.write().await;
        lk.insert(url.to_string(), (tx.clone(),uid.clone()));
    }
    
    let url_t = url.to_owned();
    let _guard = scopeguard::guard(uid.to_owned(), |uid: String| {
        tokio::spawn(async move {
            let mut lk = G_ONEBOT_RX.write().await;
            if let Some(v) = lk.get(&url_t) {
                if v.1 == uid {
                    lk.remove(&url_t);
                }
            }
        });
    });
    

    // 向onebot客户端发送生命周期包
    let life_event = kb.get_lifecycle_event().await?;
    write_half.send(tungstenite::Message::Text(life_event)).await?;

    let heartbeat = kb.get_heartbeat_event().await?;
    let tx_copy = tx.clone();
    let url_t = url.to_owned();
    let uid_t = uid.to_owned();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            let ret = tx_copy.send(heartbeat.clone()).await;
            if ret.is_err() {
                let mut lk = G_ONEBOT_RX.write().await;
                if let Some(v) = lk.get(&url_t) {
                    if v.1 == uid_t {
                        lk.remove(&url_t);
                    }
                }
                log::error!("ONEBOT_WS_REV心跳包发送出错:{}",ret.err().unwrap());
                break;
            }
        }
    });

    let url_t = url.to_owned();
    let uid_t = uid.to_owned();
    tokio::spawn(async move {
        // 将收到的事件发送到onebot客户端
        while let Some(msg) = rx.recv().await {
            let ret = write_half.send(tungstenite::Message::Text(msg)).await;
            if ret.is_err() {
                let mut lk = G_ONEBOT_RX.write().await;
                if let Some(v) = lk.get(&url_t) {
                    if v.1 == uid_t {
                        lk.remove(&url_t);
                    }
                }
                log::error!("ONEBOT_WS_REV数据发送出错:{}",ret.err().unwrap());
                break;
            }
        }
    });

    // 接收来自onebot客户端的调用
    while let Some(msg_t) = read_half.next().await {

        // 不存在连接，这退出接收
        {
            let lk = G_ONEBOT_RX.read().await;
            if let Some(v) = lk.get(url) {
                if v.1 != uid {
                    break;
                }
            }else{
                break;
            }
        }

        let msg = msg_t?;
        if ! msg.is_text() {
            continue;
        }
        let msg_text = msg.to_text()?;
        // 处理onebot的api调用
        let (_,ret) = kb.deal_onebot(url,msg_text).await;
        tx.send(ret).await?;
    }
    Ok(())
}


async fn onebot_rev_ws(ws_url:String) {
    loop {
        let mut request = tungstenite::client::IntoClientRequest::into_client_request(ws_url.clone()).unwrap();
        // 反向ws鉴权
        let g_access_token = G_ACCESS_TOKEN.read().await.clone();
        if g_access_token != "" {
            request.headers_mut().insert("Authorization", HeaderValue::from_str(&format!("Bearer {}",g_access_token)).unwrap());
        }
        let self_id = G_SELF_ID.read().await;
        request.headers_mut().append(HeaderName::from_str("X-Self-ID").unwrap(), HeaderValue::from_str(&self_id.to_string()).unwrap());
        request.headers_mut().append(HeaderName::from_str("X-Client-Role").unwrap(), HeaderValue::from_str("Universal").unwrap());
        let rst;
        if ws_url.starts_with("wss://") {
            let port_opt  = request.uri().port();
            let port;
            if port_opt.is_none() {
                port = 443;
            }else {
                port  = port_opt.unwrap().into();
            }
            let addr = format!("{}:{}",request.uri().host().unwrap(),port);
            let socket = TcpStream::connect(addr).await.unwrap();
            rst = tokio_tungstenite::client_async_tls(request, socket).await;
        }else {
            rst = tokio_tungstenite::connect_async(request).await;
        }
        
        if rst.is_err() {
            log::error!("连接到WS_REV:{ws_url} 失败");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            continue;
        }
        let (ws_stream, _) =  rst.unwrap();
        let (write_halt,read_halt) = ws_stream.split();
        let rst = deal_ws2(&ws_url,write_halt,read_halt).await;
        if rst.is_err() {
            log::error!("WS_REV:{ws_url} 断开连接");
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}


pub async fn deal_ws_rev() {
    let urls = G_REVERSE_URL.read().await.clone();
    for url in &urls {
        if !url.starts_with("ws") {
            continue;
        }
        let ws_url = url.clone();
        tokio::spawn(async {
            onebot_rev_ws(ws_url).await;
        });
    }
}