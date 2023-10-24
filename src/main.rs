

pub mod kook_onebot;
pub mod cqtool;
mod msgid_tool;
mod config_tool;
mod onebot_http;
mod onebot_ws;
mod onebot_http_rev;
mod onebot_ws_rev;

#[macro_use]
extern crate lazy_static; 

lazy_static! {
    pub static ref G_SELF_ID:RwLock<u64> = RwLock::new(0);
    pub static ref G_KOOK_TOKEN:RwLock<String> = RwLock::new(String::new());
    pub static ref G_ONEBOT_RX:RwLock<HashMap<String,(tokio::sync::mpsc::Sender<String>,String)>> = RwLock::new(HashMap::new());
    pub static ref G_ACCESS_TOKEN:RwLock<String> = RwLock::new(String::new());
    pub static ref G_SECERT:RwLock<String> = RwLock::new(String::new());
    pub static ref  G_REVERSE_URL:RwLock<Vec<String>> = RwLock::new(Vec::new());
}

use std::{collections::HashMap, sync::{Arc, atomic::AtomicI64}};
use hyper_tungstenite::hyper;
use kook_onebot::KookOnebot;
use time::UtcOffset;
use ::time::format_description;
use tokio::sync::RwLock;
use hyper::service::make_service_fn;
use crate::config_tool::read_config;


async fn connect_handle(request: hyper::Request<hyper::Body>) -> Result<hyper::Response<hyper::Body>, Box<dyn std::error::Error + Send + Sync>> {
    
    let is_pass = onebot_http::check_auth(&request).await?;

    if is_pass == false {
        log::error!("WS或HTTP鉴权失败!");
        let mut res = hyper::Response::new(hyper::Body::from(vec![]));
        *res.status_mut() = hyper::StatusCode::FORBIDDEN;
        return Ok(res);
    }
    // 处理正向ws
    if hyper_tungstenite::is_upgrade_request(&request) {
        let uid = uuid::Uuid::new_v4().to_string();
        log::warn!("接收到WS连接`{uid}`");
        let (response, websocket): (hyper::Response<hyper::Body>, hyper_tungstenite::HyperWebsocket) = hyper_tungstenite::upgrade(request, None)?;
        tokio::spawn(async move {
            let ret = onebot_ws::deal_onebot_ws(&uid,websocket).await;
            log::error!("WS断开连接:`{uid}`,`{ret:?}`");
        });
        return Ok(response);
    } else {
        // 处理正向http
        let rst = onebot_http::deal_onebot_http(request).await?;
        return Ok(rst);
    }
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    // 初始化日志
    let format = "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]";

    // 获得utc偏移
    let utc_offset;
    if let Ok(v) = UtcOffset::current_local_offset() {
        utc_offset = v;
    } else {
        // 中国是东八区，所以这里写8 hour
        utc_offset = UtcOffset::from_hms(8,0,0).unwrap();
    }

    tracing_subscriber::fmt()
    .with_timer(tracing_subscriber::fmt::time::OffsetTime::new(
        utc_offset,
        format_description::parse(format).unwrap(),
    )).with_max_level(tracing::Level::INFO)
    .init();

    log::warn!("欢迎使用KookOnebot by super1207!!! v0.0.12");

    log::warn!("开源地址:https://github.com/super1207/KookOneBot");

    log::warn!("正在加载配置文件...");

    let config_file = read_config().await.unwrap();

    let kook_token = config_file.get("kook_token").expect("配置文件缺少 kook_token 字段").as_str().unwrap();
    let mut web_host = config_file.get("web_host").expect("配置文件缺少 web_host 字段").as_str().unwrap();
    let web_port = config_file.get("web_port").expect("配置文件缺少 web_port 字段").as_u64().unwrap();
    let secret = config_file.get("secret").expect("配置文件缺少 secret 字段").as_str().unwrap();
    let access_token = config_file.get("access_token").expect("配置文件缺少 access_token 字段").as_str().unwrap();
    let reverse_url = config_file.get("reverse_uri").expect("配置文件缺少 reverse_uri 字段").as_array().unwrap();

    for url in reverse_url {
        let url_str = url.as_str().unwrap();
        G_REVERSE_URL.write().await.push(url_str.to_owned());
    }

    if web_host == "localhost" {
        web_host = "127.0.0.1";
    }

    *G_ACCESS_TOKEN.write().await = access_token.to_owned();

    *G_SECERT.write().await = secret.to_owned();


    log::warn!("加载配置文件成功");

    let mut kb = KookOnebot {
        token:kook_token.to_owned(),
        self_id:0,
        sn: Arc::new(AtomicI64::new(0)),
    };
    log::warn!("正在登录中...");
    let login_info = kb.get_login_info().await?;


    log::warn!("欢迎 `{}`({})！",login_info.nickname,login_info.user_id);
    let self_id = login_info.user_id;
    kb.self_id = self_id;
    {
        let mut lk = G_SELF_ID.write().await;
        (*lk) = kb.self_id;
        let mut lk = G_KOOK_TOKEN.write().await;
        (*lk) = kb.token.to_owned();
    }

    
    let kb_t = kb.clone();
    tokio::spawn(async move {
        loop {
            let err = kb_t.connect().await;
            log::error!("KOOK连接断开：{err:?}");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });

    
    // 反向 http 心跳
    tokio::spawn(async move {
        onebot_http_rev::deal_heartbeat(kb).await;
    });

    // 反向ws
    tokio::spawn(async move {
        onebot_ws_rev::deal_ws_rev().await;
    });


    if web_host != ""  && web_port != 0{
        let web_uri = format!("{web_host}:{web_port}");
        log::warn!("监听地址：{web_uri}");
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
    }
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}