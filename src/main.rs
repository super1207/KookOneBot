

pub mod kook_onebot;
pub mod cqtool;
mod msgid_tool;
mod config_tool;

#[macro_use]
extern crate lazy_static; 

lazy_static! {
    pub static ref G_SELF_ID:RwLock<i64> = RwLock::new(0);
    pub static ref G_KOOK_TOKEN:RwLock<String> = RwLock::new(String::new());
    pub static ref G_ONEBOT_RX:RwLock<HashMap<String,tokio::sync::mpsc::Sender<String>>> = RwLock::new(HashMap::new());
    pub static ref G_ACCESS_TOKEN:RwLock<String> = RwLock::new(String::new());
}


use std::{collections::HashMap, sync::{Arc, atomic::AtomicI64}};
use futures_util::{SinkExt, StreamExt};
use hyper_tungstenite::hyper;
use kook_onebot::KookOnebot;
use tokio::sync::RwLock;
use hyper::service::make_service_fn;

use crate::config_tool::read_config;

/// 处理ws协议
async fn serve_websocket(uid:&str,websocket: hyper_tungstenite::HyperWebsocket) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    
    let kb = crate::kook_onebot::KookOnebot {
        token: G_KOOK_TOKEN.read().await.to_owned(),
        self_id: G_SELF_ID.read().await.to_owned(),
        sn: Arc::new(AtomicI64::new(0)),
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

    // 向onebot客户端发送生命周期包
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
    
    let g_access_token = G_ACCESS_TOKEN.read().await.clone();
    if !g_access_token.is_empty() {
        let headers_map = request.headers();
        let access_token:String;
        if let Some(token) = headers_map.get("Authorization") {
            access_token = token.to_str()?.to_owned();
        }
        else {
            access_token = "".to_owned();
        }
        if access_token != "Bear ".to_owned() + &g_access_token {
            println!("ws鉴权失败!");
            let mut res = hyper::Response::new(hyper::Body::from(vec![]));
            *res.status_mut() = hyper::StatusCode::NOT_FOUND;
            return Ok(res);
        }
    }

    
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

    let config_file = read_config().await.unwrap();

    let kook_token = config_file.get("kook_token").unwrap().as_str().unwrap();
    let mut web_host = config_file.get("web_host").unwrap().as_str().unwrap();
    let web_port = config_file.get("web_port").unwrap().as_u64().unwrap();
    let access_token = config_file.get("access_token").unwrap().as_str().unwrap();

    if web_host == "localhost" {
        web_host = "127.0.0.1";
    }

    *G_ACCESS_TOKEN.write().await = access_token.to_owned();

    let mut kb = KookOnebot {
        token:kook_token.to_owned(),
        self_id:0,
        sn: Arc::new(AtomicI64::new(0)),
    };
    println!("正在登录中...");
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
    

    let web_uri = format!("{web_host}:{web_port}");
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