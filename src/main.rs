

pub mod kook_onebot;
pub mod cqtool;

#[macro_use]
extern crate lazy_static; 

lazy_static! {
    pub static ref G_SELF_ID:RwLock<i64> = RwLock::new(0);
    pub static ref G_KOOK_TOKEN:RwLock<String> = RwLock::new(String::new());
    pub static ref G_ONEBOT_RX:RwLock<HashMap<String,tokio::sync::mpsc::Sender<String>>> = RwLock::new(HashMap::new());
}


use std::collections::HashMap;
use futures_util::{SinkExt, StreamExt};
use hyper_tungstenite::hyper;
use kook_onebot::KookOnebot;
use tokio::sync::RwLock;
use hyper::service::make_service_fn;

/// 处理ws协议
async fn serve_websocket(uid:&str,websocket: hyper_tungstenite::HyperWebsocket) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    
    let kb = crate::kook_onebot::KookOnebot {
        token: G_KOOK_TOKEN.read().await.to_owned(),
        self_id: G_SELF_ID.read().await.to_owned(),
    };

    // 获得升级后的ws流
    let (tx, mut rx) =  tokio::sync::mpsc::channel::<String>(60);
    {
        let mut lk = G_ONEBOT_RX.write().await;
        lk.insert(uid.to_owned(), tx.clone());
    }
    let _guard = scopeguard::guard(uid.to_owned(), |uid: String| {
        tokio::spawn(async move {
            let mut lk = G_ONEBOT_RX.write().await;
            lk.remove(&uid);
        });
    });
    let ws_stream = websocket.await?;
    let (mut write_half, mut read_half ) = futures_util::StreamExt::split(ws_stream);

    let life_event = kb.get_lifecycle_event().await?;
    write_half.send(hyper_tungstenite::tungstenite::Message::Text(life_event)).await?;

    tokio::spawn(async move {
        // 将收到的事件发送到onebot客户端
        while let Some(msg) = rx.recv().await {
            let _ret = write_half.send(hyper_tungstenite::tungstenite::Message::Text(msg)).await;
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
            println!("deal_onebot err:{ret:?}");
        }else {
            // 发回到onebot客户端
            tx.send(ret.unwrap()).await?;
        }
    }
    Ok(())
}

async fn connect_handle(request: hyper::Request<hyper::Body>) -> Result<hyper::Response<hyper::Body>, Box<dyn std::error::Error + Send + Sync>> {
    if hyper_tungstenite::is_upgrade_request(&request) {
        let uid = uuid::Uuid::new_v4().to_string();
        println!("接收到ws连接`{uid}`");
        let (response, websocket): (hyper::Response<hyper::Body>, hyper_tungstenite::HyperWebsocket) = hyper_tungstenite::upgrade(request, None)?;
        tokio::spawn(async move {
            let ret = serve_websocket(&uid,websocket).await;
            println!("ws断开连接:`{uid}`,`{ret:?}`");
        });
        return Ok(response);
    }
    let mut res = hyper::Response::new(hyper::Body::from(vec![]));
    *res.status_mut() = hyper::StatusCode::NOT_FOUND;
    Ok(res)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut kb = KookOnebot {
        token:"1/123456=/123456789123456==".to_owned(),
        self_id:0
    };
    let login_info = kb.get_login_info().await?;
    println!("欢迎 `{}`({})！",login_info.nickname,login_info.user_id);
    let self_id = login_info.user_id;
    kb.self_id = self_id;
    {
        let mut lk = G_SELF_ID.write().await;
        (*lk) = kb.self_id;
        let mut lk = G_KOOK_TOKEN.write().await;
        (*lk) = kb.token.to_owned();
    }
    tokio::spawn(async move {
        loop {
            let err = kb.connect().await;
            println!("{err:?}");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });
    let host = "127.0.0.1";
    let port = 8080;
    let web_uri = format!("{host}:{port}");
    let addr = web_uri.parse::<std::net::SocketAddr>()?;
    let bd_rst = hyper::Server::try_bind(&addr);
    if bd_rst.is_ok() {
        // 启动服务
        let ret = bd_rst.unwrap().serve(make_service_fn(|_conn| async {
            Ok::<_, std::convert::Infallible>(hyper::service::service_fn(connect_handle))
        })).await;
        if let Err(err)  = ret{
            panic!("绑定端口号失败：{}",err)
        }
    }else {
        panic!("绑定端口号失败");
    }
    Ok(())
}