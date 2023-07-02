

pub mod kook_onebot;
pub mod cqtool;
mod msgid_tool;
mod config_tool;

#[macro_use]
extern crate lazy_static; 

lazy_static! {
    pub static ref G_SELF_ID:RwLock<u64> = RwLock::new(0);
    pub static ref G_KOOK_TOKEN:RwLock<String> = RwLock::new(String::new());
    pub static ref G_ONEBOT_RX:RwLock<HashMap<String,(tokio::sync::mpsc::Sender<String>,String)>> = RwLock::new(HashMap::new());
    pub static ref G_ACCESS_TOKEN:RwLock<String> = RwLock::new(String::new());
    pub static ref  G_REVERSE_URL:RwLock<Vec<String>> = RwLock::new(Vec::new());
}


use std::{collections::HashMap, sync::{Arc, atomic::AtomicI64}};
use futures_util::{SinkExt, StreamExt};
use hyper_tungstenite::hyper;
use kook_onebot::KookOnebot;
use tokio::sync::RwLock;
use hyper::{service::make_service_fn};
use tokio_tungstenite::connect_async;

use crate::config_tool::read_config;



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
                println!("ONEBOT_WS心跳包发送出错:{}",ret.err().unwrap());
                break;
            }
        }
    });

    tokio::spawn(async move {
        // 将收到的事件发送到onebot客户端
        while let Some(msg) = rx.recv().await {
            let ret = write_half.send(hyper_tungstenite::tungstenite::Message::Text(msg)).await;
            if ret.is_err() {
                println!("ONEBOT_WS数据发送出错:{}",ret.err().unwrap());
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
            println!("ONEBOT_WS动作调用出错:{ret:?}");
        }else {
            // 发回到onebot客户端
            tx.send(ret.unwrap()).await?;
        }
    }
    Ok(())
}


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
                println!("ONEBOT_WS_REV心跳包发送出错:{}",ret.err().unwrap());
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
                println!("ONEBOT_WS_REV数据发送出错:{}",ret.err().unwrap());
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
        let ret = kb.deal_onebot(url,msg_text).await;
        if ret.is_err() {
            println!("ONEBOT_WS_REV动作调用出错:{ret:?}");
        }else {
            // 发回到onebot客户端
            tx.send(ret.unwrap()).await?;
        }
    }
    Ok(())
}

/// 处理ws协议
async fn serve_websocket(uid:&str,websocket: hyper_tungstenite::HyperWebsocket) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream: hyper_tungstenite::WebSocketStream<hyper::upgrade::Upgraded> = websocket.await?;
    let (write_half, read_half ) = futures_util::StreamExt::split(ws_stream);
    deal_ws(uid,write_half,read_half).await?;
    Ok(())
}

fn get_params_from_uri(uri:&hyper::Uri) -> HashMap<String,String> {
    let mut ret_map = HashMap::new();
    if uri.query().is_none() {
        return ret_map;
    }
    let query_str = uri.query().unwrap();
    let query_vec = query_str.split("&");
    for it in query_vec {
        if it == "" {
            continue;
        }
        let index_opt = it.find("=");
        if index_opt.is_some() {
            let k_rst = urlencoding::decode(it.get(0..index_opt.unwrap()).unwrap());
            let v_rst = urlencoding::decode(it.get(index_opt.unwrap() + 1..).unwrap());
            if k_rst.is_err() || v_rst.is_err() {
                continue;
            }
            ret_map.insert(k_rst.unwrap().to_string(), v_rst.unwrap().to_string());
        }
        else {
            let k_rst = urlencoding::decode(it);
            if k_rst.is_err() {
                continue;
            }
            ret_map.insert(k_rst.unwrap().to_string(),"".to_owned());
        }
    }
    ret_map
}

async fn connect_handle(mut request: hyper::Request<hyper::Body>) -> Result<hyper::Response<hyper::Body>, Box<dyn std::error::Error + Send + Sync>> {
    
    // 获得当前的访问密钥
    let mut is_pass = false;
    let g_access_token = G_ACCESS_TOKEN.read().await.clone();
    let headers_map = request.headers();
    if !g_access_token.is_empty() {
        {
            let access_token:String;
            if let Some(token) = headers_map.get("Authorization") {
                access_token = token.to_str()?.to_owned();
            }
            else {
                access_token = "".to_owned();
            }
            if access_token == "Bear ".to_owned() + &g_access_token {
                is_pass = true;
            }
        }
        {
            let uri = request.uri().clone();
            let mp = get_params_from_uri(&uri);
            if let Some(val) = mp.get("access_token") {
                if &g_access_token == val {
                    is_pass = true;
                }
            }
            
        }

    } else {
        is_pass = true;
    }

    if is_pass == false {
        println!("WS鉴权失败!");
        let mut res = hyper::Response::new(hyper::Body::from(vec![]));
        *res.status_mut() = hyper::StatusCode::NOT_FOUND;
        return Ok(res);
    }

    let url_path = request.uri().path().to_owned();

    if hyper_tungstenite::is_upgrade_request(&request) {
        let uid = uuid::Uuid::new_v4().to_string();
        println!("接收到WS连接`{uid}`");
        let (response, websocket): (hyper::Response<hyper::Body>, hyper_tungstenite::HyperWebsocket) = hyper_tungstenite::upgrade(request, None)?;
        tokio::spawn(async move {
            let ret = serve_websocket(&uid,websocket).await;
            println!("WS断开连接:`{uid}`,`{ret:?}`");
        });
        return Ok(response);
    } else {
        
        let action = url_path.get(1..).ok_or("get action from url_path err")?;
        let kb = crate::kook_onebot::KookOnebot {
            token: G_KOOK_TOKEN.read().await.to_owned(),
            self_id: G_SELF_ID.read().await.to_owned(),
            sn: Arc::new(AtomicI64::new(0)),
        };
        let method = request.method().to_string();
        let params;
        if method == "GET" {
            let mp = get_params_from_uri(request.uri());
            params = serde_json::json!(mp);
        }else if method == "POST" {
            if let Some(content_type) = headers_map.get("content-type") {
                if content_type.to_str()? == "application/json" {
                    let body = hyper::body::to_bytes(request.body_mut()).await?;
                    params  = serde_json::from_slice(&body)?;
                } else {
                    let body = hyper::body::to_bytes(request.body_mut()).await?;
                    params = url::form_urlencoded::parse(&body).collect::<serde_json::Value>();
                }
            } else {
                let body = hyper::body::to_bytes(request.body_mut()).await?;
                params = url::form_urlencoded::parse(&body).collect::<serde_json::Value>();
            }
        } else {
            let res = hyper::Response::new(hyper::Body::from(vec![]));
            return Ok(res);
        }
        let js = serde_json::json!({
            "action":action,
            "params": params
        });
        let ret = kb.deal_onebot("", &js.to_string()).await?;
        let mut res = hyper::Response::new(hyper::Body::from(ret));
        res.headers_mut().insert("Content-Type", hyper::http::HeaderValue::from_static("application/json"));
        return Ok(res);
    }
    
    // let mut res = hyper::Response::new(hyper::Body::from(vec![]));
    // *res.status_mut() = hyper::StatusCode::NOT_FOUND;
    // Ok(res)
}


async fn onebot_rev_ws(ws_url:String) {
    loop {
        let rst = connect_async(ws_url.clone()).await;
        if rst.is_err() {
            println!("连接到WS_REV:{ws_url} 失败");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            continue;
        }
        let (ws_stream, _) =  rst.unwrap();
        let (write_halt,read_halt) = ws_stream.split();
        let rst = deal_ws2(&ws_url,write_halt,read_halt).await;
        if rst.is_err() {
            println!("WS_REV:{ws_url} 断开连接");
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    println!("欢迎使用KookOnebot by super1207!!! v0.0.4");

    println!("正在加载配置文件...");

    let config_file = read_config().await.unwrap();

    let kook_token = config_file.get("kook_token").unwrap().as_str().unwrap();
    let mut web_host = config_file.get("web_host").unwrap().as_str().unwrap();
    let web_port = config_file.get("web_port").unwrap().as_u64().unwrap();
    let access_token = config_file.get("access_token").unwrap().as_str().unwrap();
    let reverse_url = config_file.get("reverse_uri").unwrap().as_array().unwrap();
    for url in reverse_url {
        let url_str = url.as_str().unwrap();
        G_REVERSE_URL.write().await.push(url_str.to_owned());
    }

    if web_host == "localhost" {
        web_host = "127.0.0.1";
    }

    *G_ACCESS_TOKEN.write().await = access_token.to_owned();


    println!("加载配置文件成功");

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

    let kb2 = kb.clone();
    tokio::spawn(async move {
        loop {
            let err = kb.connect().await;
            println!("{err:?}");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });

    
    // http 心跳
    tokio::spawn(async move {
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
    });

    // 反向ws
    tokio::spawn(async move {
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
    });


    if web_host != ""  && web_port != 0{
        let web_uri = format!("{web_host}:{web_port}");
        println!("监听地址：{web_uri}");
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