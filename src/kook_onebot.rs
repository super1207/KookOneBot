

use std::{str::FromStr, io::Read, collections::{HashMap, VecDeque}, time::Duration, sync::{atomic::AtomicI64, Arc}, path::Path};

use flate2::read::ZlibDecoder;
use futures_util::{StreamExt, SinkExt};
use hyper::http::{HeaderName, HeaderValue};
use regex::Regex;
use serde_derive::{Serialize, Deserialize};
use tokio_tungstenite::connect_async;
use std::time::SystemTime;
use crate::{G_ONEBOT_RX, cqtool::{str_msg_to_arr, arr_to_cq_str, cq_params_encode, make_kook_text}, msgid_tool::QMessageStruct, G_REVERSE_URL};


#[derive(Clone)]
pub(crate) struct KookOnebot {
    pub token:String,
    pub self_id:u64,
    pub sn:Arc<AtomicI64>
}

impl KookOnebot {



    pub async fn post_to_client(&self,url:&str,json_str:&str) -> Result<(), Box<dyn std::error::Error + Send + Sync>>{
        let uri = reqwest::Url::from_str(url)?;
        let client = reqwest::Client::builder().danger_accept_invalid_certs(true).no_proxy().build()?;
        let mut req = client.post(uri).body(reqwest::Body::from(json_str.to_owned())).build()?;
        req.headers_mut().append(HeaderName::from_str("Content-type")?, HeaderValue::from_str("application/json")?);
        req.headers_mut().append(HeaderName::from_str("X-Self-ID")?, HeaderValue::from_str(&self.self_id.to_string())?);
        client.execute(req).await?;
        Ok(())
    }


    async fn send_to_onebot_client(&self,json_str:&str) {
        println!("发送ONEBOT事件:{json_str}");
        {
            let lk = G_ONEBOT_RX.read().await;
            for (_,v) in &*lk {
                let rst = v.send(json_str.to_string()).await;
                if rst.is_err() {
                    println!("发送事件到ONEBOT_WS客户端出错:`{}`",rst.err().unwrap());
                }
            }
        }
        let lk = G_REVERSE_URL.read().await;
        for uri in &*lk {
            if !uri.starts_with("http") {
                continue;
            }
            let rst = self.post_to_client(uri,json_str).await;
            if rst.is_err() {
                println!("发送事件到ONEBOT_HTTP客户端出错:`{}`",rst.err().unwrap());
            }
        }
        
    }

    async fn http_get_json(&self,uri:&str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        println!("发送KOOK_GET:{uri}");
        let uri = reqwest::Url::from_str(&format!("https://www.kookapp.cn/api/v3{uri}"))?;
        let client = reqwest::Client::builder().danger_accept_invalid_certs(true).no_proxy().build()?;
        let mut req = client.get(uri).build()?;
        let token = &self.token;
        req.headers_mut().append(HeaderName::from_str("Authorization")?, HeaderValue::from_str(&format!("Bot {token}"))?);
        let ret = client.execute(req).await?;
        let retbin = ret.bytes().await?.to_vec();
        let ret_str = String::from_utf8(retbin)?;
        println!("KOOK_GET响应:{ret_str}");
        let js:serde_json::Value = serde_json::from_str(&ret_str)?;
        let ret = js.get("data").ok_or("get data err")?;
        Ok(ret.to_owned())
    }

    async fn http_post_json(&self,uri:&str,json:&serde_json::Value) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>{
        let json_str = json.to_string();
        println!("发送KOOK_POST:{uri}\n{}",json_str);
        let uri = reqwest::Url::from_str(&format!("https://www.kookapp.cn/api/v3{uri}"))?;
        let client = reqwest::Client::builder().danger_accept_invalid_certs(true).no_proxy().build()?;
        let mut req = client.post(uri).body(reqwest::Body::from(json_str)).build()?;
        let token = &self.token;
        req.headers_mut().append(HeaderName::from_str("Authorization")?, HeaderValue::from_str(&format!("Bot {token}"))?);
        req.headers_mut().append(HeaderName::from_str("Content-type")?, HeaderValue::from_str("application/json")?);
        let ret = client.execute(req).await?;
        let retbin = ret.bytes().await?.to_vec();
        let ret_str = String::from_utf8(retbin)?;
        println!("KOOK_POST响应:{ret_str}");
        let js:serde_json::Value = serde_json::from_str(&ret_str)?;
        let ret = js.get("data").ok_or("get data err")?;
        Ok(ret.to_owned())
    }

    #[allow(dead_code)]
    async fn get_group_list(&self)-> Result<Vec<GroupInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let ret_json = self.http_get_json("/guild/list").await?;
        let guild_arr = ret_json.get("items").ok_or("get items err")?.as_array().ok_or("items not arr")?;
        let mut guild_arr_t = vec![];
        let mut ret_arr = vec![];
        for it in guild_arr {
            let id = it.get("id").ok_or("get id err")?.as_str().ok_or("id not str")?;
            guild_arr_t.push(id.to_string());
        }
        for it in guild_arr_t {
            let ret_json = self.http_get_json(&format!("/channel/list?guild_id={it}")).await?;
            let channel_arr = ret_json.get("items").ok_or("get items err")?.as_array().ok_or("items not arr")?;
            for it2 in channel_arr {
                let id = it2.get("id").ok_or("get id err")?.as_str().ok_or("id not str")?;
                // let id2 = format!("{it}-{id}");
                let group_name = it2.get("name").ok_or("get name err")?.as_str().ok_or("name not str")?;

                let tp = it2.get("type").ok_or("get type err")?.as_i64().ok_or("type not i64")?;
                let is_category = it2.get("is_category").ok_or("get is_category err")?.as_bool().ok_or("is_category not bool")?;

                if !is_category && tp == 1 {
                    ret_arr.push(GroupInfo {
                        group_id:id.parse::<u64>()?,
                        group_name:group_name.to_owned(),
                        member_count:0,
                        max_member_count:0
                    });
                }
            }
        }
        Ok(ret_arr)
    }

    #[allow(dead_code)]
    async fn get_channel_list(&self,guild_id:&str)-> Result<Vec<GroupInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let mut ret_arr = vec![];
        let ret_json = self.http_get_json(&format!("/channel/list?guild_id={guild_id}")).await?;
        let channel_arr = ret_json.get("items").ok_or("get items err")?.as_array().ok_or("items not arr")?;
        for it2 in channel_arr {
            let id = it2.get("id").ok_or("get id err")?.as_str().ok_or("id not str")?;
            // let id2 = format!("{it}-{id}");
            let group_name = it2.get("name").ok_or("get name err")?.as_str().ok_or("name not str")?;

            let tp = it2.get("type").ok_or("get type err")?.as_i64().ok_or("type not i64")?;
            let is_category = it2.get("is_category").ok_or("get is_category err")?.as_bool().ok_or("is_category not bool")?;

            if !is_category && tp == 1 {
                ret_arr.push(GroupInfo {
                    group_id:id.parse::<u64>()?,
                    group_name:group_name.to_owned(),
                    member_count:0,
                    max_member_count:0
                });
            }
        }
        Ok(ret_arr)
    }

    #[allow(dead_code)]
    pub async fn get_login_info(&self)-> Result<LoginInfo, Box<dyn std::error::Error + Send + Sync>> {
        let login_info = self.http_get_json("/user/me").await?;
        let user_id = login_info.get("id").ok_or("get id err")?.as_str().ok_or("id not str")?;
        let nickname = login_info.get("username").ok_or("get username err")?.as_str().ok_or("username not str")?;
        Ok(LoginInfo {
            user_id:user_id.parse::<u64>()?,
            nickname:nickname.to_owned()
        })
    }

    async fn http_post(url:&str,data:Vec<u8>,headers:&HashMap<String, String>,is_post:bool) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let client;
        let uri = reqwest::Url::from_str(url)?;
        if uri.scheme() == "http" {
            client = reqwest::Client::builder().no_proxy().build()?;
        } else {
            client = reqwest::Client::builder().danger_accept_invalid_certs(true).no_proxy().build()?;
        }
        let mut req;
        if is_post {
            req = client.post(uri).body(reqwest::Body::from(data)).build()?;
        }else {
            req = client.get(uri).build()?;
        }
        for (key,val) in headers {
            req.headers_mut().append(HeaderName::from_str(key)?, HeaderValue::from_str(val)?);
        }
        let retbin;
        let ret = client.execute(req).await?;
        retbin = ret.bytes().await?.to_vec();
        return Ok(retbin);
    }

    #[allow(dead_code)]
    pub async fn upload_image(&self,uri:&str)-> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let file_bin;
        if uri.starts_with("http") {
            file_bin = Self::http_post(uri,vec![],&HashMap::new(),false).await?;
        }else if uri.starts_with("base64://") {
            let b64_str = uri.get(9..).unwrap();
            file_bin = base64::Engine::decode(&base64::engine::GeneralPurpose::new(
                &base64::alphabet::STANDARD,
                base64::engine::general_purpose::PAD), b64_str)?;
           
        }else {
            let file_path = uri.get(8..).ok_or("can't get file_path")?;
            let path = Path::new(&file_path);
            file_bin = tokio::fs::read(path).await?;
        }
        
        let uri = reqwest::Url::from_str(&format!("https://www.kookapp.cn/api/v3/asset/create"))?;
        let client = reqwest::Client::builder().danger_accept_invalid_certs(true).no_proxy().build()?;
        let form = reqwest::multipart::Form::new().part("file", reqwest::multipart::Part::bytes(file_bin).file_name("test"));
        let mut req = client.post(uri).multipart(form).build()?;
        let token = &self.token;
        req.headers_mut().append(HeaderName::from_str("Authorization")?, HeaderValue::from_str(&format!("Bot {token}"))?);
        let ret = client.execute(req).await?;
        let retbin = ret.bytes().await?.to_vec();
        let ret_str = String::from_utf8(retbin)?;
        let js:serde_json::Value = serde_json::from_str(&ret_str)?;
        let ret = js.get("data").ok_or("get data err")?.get("url").ok_or("url not found")?.as_str().ok_or("url not str")?;
        Ok(ret.to_owned())
    }

    #[allow(dead_code)]
    async fn get_stranger_info(&self,user_id:&str)-> Result<StrangerInfo, Box<dyn std::error::Error + Send + Sync>> {
        let stranger_info = self.http_get_json(&format!("/user/view?user_id={user_id}")).await?;
        let user_id = stranger_info.get("id").ok_or("get id err")?.as_str().ok_or("id not str")?;
        let nickname = stranger_info.get("username").ok_or("get username err")?.as_str().ok_or("username not str")?;
        Ok(StrangerInfo {
            user_id:user_id.parse::<u64>()?,
            nickname:nickname.to_owned(),
            sex:"unknown".to_owned(),
            age:0
        })
    }

    #[allow(dead_code)]
    async fn get_group_info(&self,group_id:&str)-> Result<GroupInfo, Box<dyn std::error::Error + Send + Sync>> {
        let stranger_info = self.http_get_json(&format!("/channel/view?target_id={group_id}")).await?;
        let group_id = stranger_info.get("id").ok_or("get id err")?.as_str().ok_or("id not str")?;
        let group_name = stranger_info.get("name").ok_or("get name err")?.as_str().ok_or("name not str")?;
        Ok(GroupInfo {
            group_id:group_id.parse::<u64>()?,
            group_name:group_name.to_owned(),
            member_count:0,
            max_member_count:0
        })
    }

    #[allow(dead_code)]
    async fn get_msg(&self,msg_id:&str)-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _msg_info = self.http_get_json(&format!("/message/view?msg_id={msg_id}")).await?;
        // println!("msg_info:{}",msg_info.to_string());
        Ok(())
    }

    #[allow(dead_code)]
    async fn get_group_member_info(&self,group_id:&str,user_id:&str)-> Result<GroupMemberInfo, Box<dyn std::error::Error + Send + Sync>> {
        let group_info = self.http_get_json(&format!("/channel/view?target_id={group_id}")).await?;
        let guild_id = group_info.get("guild_id").ok_or("get guild_id err")?.as_str().ok_or("guild_id not str")?;
        let stranger_info = self.http_get_json(&format!("/user/view?user_id={user_id}&guild_id={guild_id}")).await?;
        let guild_info = self.http_get_json(&format!("/guild/view?guild_id={guild_id}")).await?;
        let owner_id = guild_info.get("user_id").ok_or("get user_id err")?.as_str().ok_or("user_id not str")?;
        let role;
        if owner_id == user_id {
            role = "owner";
        }else {
            let roles = stranger_info.get("roles").ok_or("get roles err")?.as_array().ok_or("roles not arr")?;
            // println!("roles:{:?}",roles);
            if roles.len() != 0 { 
                role = "admin";
            } else {
                role = "member";
            }
        }
        Ok(GroupMemberInfo {
            group_id:group_id.parse::<u64>()?,
            user_id:user_id.parse::<u64>()?,
            nickname:stranger_info.get("username").ok_or("get username err")?.as_str().ok_or("username not str")?.to_owned(),
            card:stranger_info.get("nickname").ok_or("get nickname err")?.as_str().ok_or("nickname not str")?.to_owned(),
            sex:"unknown".to_owned(),
            age:0,
            area:"".to_owned(),
            join_time:(stranger_info.get("joined_at").ok_or("get joined_at err")?.as_u64().ok_or("joined_at not u64")? / 1000) as i32,
            last_sent_time:(stranger_info.get("active_time").ok_or("get active_time err")?.as_u64().ok_or("active_time not u64")? / 1000) as i32,
            level:"".to_owned(),
            role:role.to_owned(),
            unfriendly:false,
            title:"".to_owned(),
            title_expire_time:0,
            card_changeable:false
        })
    }

    #[allow(dead_code)]
    async fn set_group_kick(&self,group_id:&str,user_id:&str)-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let group_info = self.http_get_json(&format!("/channel/view?target_id={group_id}")).await?;
        let guild_id = group_info.get("guild_id").ok_or("get guild_id err")?.as_str().ok_or("guild_id not str")?;
        let mut json:serde_json::Value = serde_json::from_str("{}")?;
        json["guild_id"] = guild_id.into();
        json["target_id"] = user_id.into();
        let _ret_json = self.http_post_json("/guild/kickout",&json).await?;
        Ok(())
    }

    async fn delete_msg(&self,msg_id:&str)-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut json:serde_json::Value = serde_json::from_str("{}")?;
        json["msg_id"] = msg_id.into();
        let _ret_json = self.http_post_json("/message/delete",&json).await?;
        Ok(())
    }

    async fn set_group_leave(&self,group_id:&str)-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let group_info = self.http_get_json(&format!("/channel/view?target_id={group_id}")).await?;
        let guild_id = group_info.get("guild_id").ok_or("get guild_id err")?.as_str().ok_or("guild_id not str")?;
        let mut json:serde_json::Value = serde_json::from_str("{}")?;
        json["guild_id"] = guild_id.into();
        let _ret_json = self.http_post_json("/guild/leave",&json).await?;
        Ok(())
    }

    pub async fn get_friend_list(&self)-> Result<Vec<FriendInfo>, Box<dyn std::error::Error + Send + Sync>> {

        let mut ret_vec = vec![];
        let friend_list = self.http_get_json(&format!("/user-chat/list")).await?;
        for it in friend_list.get("items").ok_or("items not found")?.as_array().ok_or("items not arr")? {
            let target_info = it.get("target_info").ok_or("target_info not found")?;
            let id = target_info.get("id").ok_or("id not found")?.as_str().ok_or("id not str")?;
            let username = target_info.get("username").ok_or("username not found")?.as_str().ok_or("username not str")?;
            ret_vec.push(FriendInfo {
                user_id: id.parse::<u64>()?,
                nickname: username.to_owned(),
                remark: username.to_owned()
            });
        }
        let meta = friend_list.get("meta").ok_or("meta not found")?;
        let page_total = meta.get("page_total").ok_or("page_total not found")?.as_i64().ok_or("page_total not i32")?;
        for page in 1..page_total{
            let friend_list = self.http_get_json(&format!("/user-chat/list?page={page}")).await?;
            for it in friend_list.get("items").ok_or("items not found")?.as_array().ok_or("items not arr")? {
                let target_info = it.get("target_info").ok_or("target_info not found")?;
                let id = target_info.get("id").ok_or("id not found")?.as_str().ok_or("id not str")?;
                let username = target_info.get("username").ok_or("username not found")?.as_str().ok_or("username not str")?;
                ret_vec.push(FriendInfo {
                    user_id: id.parse::<u64>()?,
                    nickname: username.to_owned(),
                    remark: username.to_owned()
                });
            }
        }
        Ok(ret_vec)
    }

    async fn set_group_name(&self,group_id:&str,name:&str)-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut json:serde_json::Value = serde_json::from_str("{}")?;
        json["channel_id"] = group_id.into();
        json["name"] = name.into();
        let _ret_json = self.http_post_json("/channel/update",&json).await?;
        Ok(())
    }

    async fn set_group_card(&self,group_id:&str,user_id:&str,card:&str)-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let group_info = self.http_get_json(&format!("/channel/view?target_id={group_id}")).await?;
        let guild_id = group_info.get("guild_id").ok_or("get guild_id err")?.as_str().ok_or("guild_id not str")?;
        let mut json:serde_json::Value = serde_json::from_str("{}")?;
        json["guild_id"] = guild_id.into();
        json["user_id"] = user_id.into();
        json["nickname"] = card.into();
        let _ret_json = self.http_post_json("/guild/nickname",&json).await?;
        Ok(())
    }

    // #[allow(dead_code)]
    // pub async fn get_friend_list(&self)-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    //     let friend_list = self.http_get_json(&format!("/user-chat/list")).await?;
    //     println!("friend_list:{}",friend_list.to_string());
    //     Ok(())
    // }

    #[allow(dead_code)]
    async fn send_group_msg(&self,tp:i32,group_id:&str,message:&str)-> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut json:serde_json::Value = serde_json::from_str("{}")?;
        json["content"] = message.into();
        json["target_id"] = group_id.into();
        json["type"] = tp.into();
        let ret_json = self.http_post_json("/message/create",&json).await?;
        let msg_id = ret_json.get("msg_id").ok_or("msg_id not found")?.as_str().ok_or("msg_id not str")?;
        Ok(msg_id.to_owned())
    }

    #[allow(dead_code)]
    async fn send_private_msg(&self,tp:i32,user_id:&str,message:&str)-> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut json:serde_json::Value = serde_json::from_str("{}")?;
        json["content"] = message.into();
        json["target_id"] = user_id.into();
        json["type"] = tp.into();
        let ret_json = self.http_post_json("/direct-message/create",&json).await?;
        let msg_id = ret_json.get("msg_id").ok_or("msg_id not found")?.as_str().ok_or("msg_id not str")?;
        Ok(msg_id.to_owned())
    }


    async fn get_gateway(&self)-> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let ret_json = self.http_get_json(&format!("/gateway/index?compress=1")).await?;
        Ok(ret_json.get("url").ok_or("get url err")?.as_str().ok_or("url not str")?.to_owned())
    }

    pub async fn connect(&self)-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let wss_url = self.get_gateway().await?;
        println!("正在连接KOOK端口...");
        let (ws_stream, _) = connect_async(wss_url).await?;
        let (mut write_halt,mut read_halt) = ws_stream.split();
        let sn_ptr = self.sn.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let json_str = serde_json::json!({
                    "s": 2,
                    "sn": sn_ptr.load(std::sync::atomic::Ordering::Relaxed)
                }).to_string();
                println!("发送KOOK心跳:{json_str}");
                let foo = write_halt.send(tungstenite::Message::Text(json_str)).await;
                if foo.is_err() {
                    println!("发送KOOK心跳失败");
                    break;
                }
            }
        });
        while let Some(msg) = read_halt.next().await {
            let raw_msg = msg?;
            if raw_msg.is_binary() {
                // kook返回的数据是压缩的，需要先解压
                let bin = raw_msg.into_data();
                let mut d = ZlibDecoder::new(bin.as_slice());
                let mut s = String::new();
                d.read_to_string(&mut s).unwrap();
                let js:serde_json::Value = serde_json::from_str(&s)?;
    
                let s = js.get("s").ok_or("s not found")?.as_i64().ok_or("s not i64")?;
                if s == 5 {
                    println!("正在重连KOOK");
                    break;
                }
                else if s == 1 {
                    println!("连接KOOK成功");
                }
                else if s == 0 {
                    println!("收到KOOK事件:{}",js.to_string());
                    let d = js.get("d").ok_or("d not found")?;
                    let sn = js.get("sn").ok_or("sn not found")?.as_i64().ok_or("sn not i64")?;
                    self.sn.store(sn, std::sync::atomic::Ordering::Relaxed);
                    let rst = self.deal_kook_event(d.clone()).await;
                    if rst.is_err() {
                        println!("处理KOOK事件出错:{}",rst.err().unwrap());
                    }
                }else if s == 3 {
                    println!("收到KOOK心跳响应包");
                } else {
                    println!("收到未知的KOOK数据:{}",js.to_string());
                }
            }
        }
        Ok(())
    }
    pub async fn get_lifecycle_event(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let self_id = self.self_id;
        let tm = SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs().to_string();
        let ret = format!("{{\"meta_event_type\":\"lifecycle\",\"post_type\":\"meta_event\",\"self_id\":{self_id},\"sub_type\":\"connect\",\"time\":{tm}}}");
        Ok(ret)
    }
    pub async fn get_heartbeat_event(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let self_id = self.self_id;
        let tm = SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs().to_string();
        let js = serde_json::json!({
            "time":tm,
            "self_id":self_id,
            "post_type":"meta_event",
            "meta_event_type":"heartbeat",
            "interval":5000,
            "status":{
                "online":true,
                "good":true
            }
        });
        Ok(js.to_string())
    }
    async fn deal_group_message_event(&self,data:&serde_json::Value,user_id:u64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let group_id_str = data.get("target_id").ok_or("target_id not found")?.as_str().ok_or("target_id not str")?;
        let group_id = group_id_str.parse::<u64>()?;
        let mut message = data.get("content").ok_or("content not found")?.as_str().ok_or("content not str")?.to_owned();
        

        async fn get_sender(self_t:&KookOnebot,group_id:u64,user_id: u64) -> Result<GroupMemberInfo, Box<dyn std::error::Error + Send + Sync>> {
            lazy_static! {
                static ref CACHE : std::sync::RwLock<VecDeque<(u64,u64,GroupMemberInfo,u64)>>  = std::sync::RwLock::new(VecDeque::from([]));
            }
            let tm = SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
            {
                let mut lk = CACHE.write().unwrap();
                
                loop {
                    let mut remove_index = 0;
                    for it in &*lk {
                        remove_index += 1;
                        if tm - it.3 > 60 {
                            break;
                        }
                    }
                    if remove_index == lk.len() {
                        break;
                    }
                    lk.remove(remove_index - 1);
                }
                
                for it in &*lk {
                    if it.0 == group_id && it.1 == user_id{
                        return Ok(it.2.clone());
                    }
                }
            }
            
            let info = self_t.get_group_member_info(&group_id.to_string(),&user_id.to_string()).await?;
            {
                let mut lk = CACHE.write().unwrap();
                lk.push_back((group_id,user_id,info.clone(),tm));
            }
            Ok(info)
        }

        
        let sender: GroupMemberInfo = get_sender(self,group_id,user_id).await?;
        // let message_id_str = data.get("msg_id").ok_or("msg_id not found")?.as_str().ok_or("msg_id not str")?;
        let msg_type = data.get("type").ok_or("type not found")?.as_i64().ok_or("type not i64")?;
        if msg_type == 2 { // 图片消息
            message = format!("[CQ:image,file={},url={}]",cq_params_encode(&message),cq_params_encode(&message));
        }

        if msg_type == 10 { // 卡牌消息
            #[derive(Serialize)]
            struct FileInfo {
                url:String,
                name:String,
                size:i64,
                busid:i64
            }
            fn get_file(message:&str) -> Result<Option<FileInfo>, Box<dyn std::error::Error + Send + Sync>> {
                let err = "get file err";
                let js_arr:serde_json::Value = serde_json::from_str(&message)?;
                let card_arr = js_arr.as_array().ok_or(err)?;
                if card_arr.len() != 1 {
                    return Ok(None);
                }
                let md_arr = card_arr.get(0).unwrap().get("modules").ok_or(err)?.as_array().ok_or(err)?;
                if md_arr.len() != 1 {
                    return Ok(None);
                }
                let obj = md_arr.get(0).unwrap();
                let tp = obj.get("type").ok_or(err)?.as_str().ok_or(err)?;
                if tp != "file" {
                    return Ok(None);
                }
                let url = obj.get("src").ok_or(err)?.as_str().ok_or(err)?.to_owned();
                if !url.starts_with("https://img.kookapp.cn/") {
                    return Ok(None);
                }
                let name = obj.get("title").ok_or(err)?.as_str().ok_or(err)?.to_owned();
                let size = obj.get("size").ok_or(err)?.as_i64().ok_or(err)?.to_owned();
                return  Ok(Some(FileInfo{
                    url,
                    name,
                    size,
                    busid:0
                }));
            }
            
            // 处理文件
            if let Ok(file) = get_file(&message) {
                if let Some(f) = file {
                    let  event_json = serde_json::json!({
                        "time":SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
                        "self_id":self.self_id,
                        "post_type":"notice",
                        "notice_type":"group_upload",
                        "group_id":group_id,
                        "user_id":user_id,
                        "file":f
                    });
                    self.send_to_onebot_client(&event_json.to_string()).await;
                    return Ok(())
                }
            }
            
        }

        fn reformat_dates(before: &str) -> String {
            lazy_static! {
                static ref AT_REGEX : Regex = Regex::new(
                    r"\(met\)(?P<qq>(\d+)|(all))\(met\)"
                    ).unwrap();
            }
            AT_REGEX.replace_all(before, "[CQ:at,qq=$qq]").to_string()
        }
        let raw_msg_id = data.get("msg_id").ok_or("msg_id not found")?.as_str().ok_or("msg_id not str")?;
        let msg_id = crate::msgid_tool::add_msg_id(QMessageStruct {raw_ids:vec![raw_msg_id.to_owned()], user_id });
        let msg = reformat_dates(&message);
        let  event_json = serde_json::json!({
            "time":SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
            "self_id":self.self_id,
            "post_type":"message",
            "message_type":"group",
            "sub_type":"normal",
            "message_id":msg_id,
            "group_id":group_id,
            "user_id":user_id,
            "message":msg,
            "raw_message":msg,
            "font":0,
            "sender":sender
        });
        self.send_to_onebot_client(&event_json.to_string()).await;
        Ok(())
    }

    async fn deal_private_message_event(&self,data:&serde_json::Value,user_id:u64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut message = data.get("content").ok_or("content not found")?.as_str().ok_or("content not str")?.to_owned();
        
        let extra = data.get("extra").ok_or("extra not found")?;
        let author = extra.get("author").ok_or("author not found")?;

        let username = author.get("username").ok_or("username not found")?.as_str().ok_or("username not str")?;

        let sender: FriendInfo = FriendInfo {
            user_id,
            nickname: username.to_owned(),
            remark: username.to_owned(),
        };

        // let message_id_str = data.get("msg_id").ok_or("msg_id not found")?.as_str().ok_or("msg_id not str")?;
        let msg_type = data.get("type").ok_or("type not found")?.as_i64().ok_or("type not i64")?;
        if msg_type == 2 { // 图片消息
            message = format!("[CQ:image,file={},url={}]",cq_params_encode(&message),cq_params_encode(&message));
        }

        fn reformat_dates(before: &str) -> String {
            lazy_static! {
                static ref AT_REGEX : Regex = Regex::new(
                    r"\(met\)(?P<qq>(\d+)|(all))\(met\)"
                    ).unwrap();
            }
            AT_REGEX.replace_all(before, "[CQ:at,qq=$qq]").to_string()
        }
        let raw_msg_id = data.get("msg_id").ok_or("msg_id not found")?.as_str().ok_or("msg_id not str")?;
        let msg_id = crate::msgid_tool::add_msg_id(QMessageStruct {raw_ids:vec![raw_msg_id.to_owned()], user_id });
        let msg = reformat_dates(&message);
        let  event_json = serde_json::json!({
            "time":SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
            "self_id":self.self_id,
            "post_type":"message",
            "message_type":"private",
            "sub_type":"friend",
            "message_id":msg_id,
            "user_id":user_id,
            "message":msg,
            "raw_message":msg,
            "font":0,
            "sender":sender
        });
        self.send_to_onebot_client(&event_json.to_string()).await;
        Ok(())
    }


    async fn deal_group_decrease_event(&self,data:&serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let guild_id_str = data.get("target_id").ok_or("target_id not found")?.as_str().ok_or("target_id not str")?;
        let group_list = self.get_channel_list(guild_id_str).await?;
        let user_id_str = data.get("extra").ok_or("extra not found")?
                                .get("body").ok_or("body not found")?
                                .get("user_id").ok_or("user_id not found")?
                                .as_str().ok_or("user_id not str")?;
        let user_id = user_id_str.parse::<u64>()?;
        for it in &group_list {
            
            let  event_json = serde_json::json!({
                "time":SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
                "self_id":self.self_id,
                "post_type":"notice",
                "notice_type":"group_decrease",
                "sub_type":"leave",
                "group_id":it.group_id,
                "operator_id":user_id,
                "user_id":user_id,
            });
            self.send_to_onebot_client(&event_json.to_string()).await;
        }
        Ok(())
    }

    async fn deal_group_recall(&self,data:&serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let msg_id_str = data.get("extra").ok_or("extra not found")?
                                .get("body").ok_or("body not found")?
                                .get("msg_id").ok_or("msg_id not found")?
                                .as_str().ok_or("msg_id not str")?;
        let group_id_str = data.get("extra").ok_or("extra not found")?
                                .get("body").ok_or("body not found")?
                                .get("channel_id").ok_or("channel_id not found")?
                                .as_str().ok_or("channel_id not str")?;
        let group_id = group_id_str.parse::<u64>()?;
        let (cq_id,user_id) = crate::msgid_tool::get_cq_msg_id(msg_id_str);
        // self.get_msg(msg_id_str).await?;
        let  event_json = serde_json::json!({
            "time":SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
            "self_id":self.self_id,
            "post_type":"notice",
            "notice_type":"group_recall",
            "group_id":group_id,
            "user_id": user_id,
            "operator_id":1,
            "message_id": cq_id
        });
        self.send_to_onebot_client(&event_json.to_string()).await;
        Ok(())
    }


    async fn deal_private_recall(&self,data:&serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let msg_id_str = data.get("extra").ok_or("extra not found")?
                                .get("body").ok_or("body not found")?
                                .get("msg_id").ok_or("msg_id not found")?
                                .as_str().ok_or("msg_id not str")?;
        let user_id_str = data.get("extra").ok_or("extra not found")?
                                .get("body").ok_or("body not found")?
                                .get("author_id").ok_or("author_id not found")?
                                .as_str().ok_or("author_id not str")?;
        let user_id = user_id_str.parse::<u64>()?;
        let (cq_id,_user_id) = crate::msgid_tool::get_cq_msg_id(msg_id_str);
        // self.get_msg(msg_id_str).await?;
        let  event_json = serde_json::json!({
            "time":SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
            "self_id":self.self_id,
            "post_type":"notice",
            "notice_type":"friend_recall",
            "user_id": user_id,
            "message_id": cq_id
        });
        self.send_to_onebot_client(&event_json.to_string()).await;
        Ok(())
    }



    async fn deal_group_increase_event(&self,data:&serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let guild_id_str = data.get("target_id").ok_or("target_id not found")?.as_str().ok_or("target_id not str")?;
        let group_list = self.get_channel_list(guild_id_str).await?;
        let user_id_str = data.get("extra").ok_or("extra not found")?
                                .get("body").ok_or("body not found")?
                                .get("user_id").ok_or("user_id not found")?
                                .as_str().ok_or("user_id not str")?;
        let user_id = user_id_str.parse::<u64>()?;
        for it in &group_list {
            
            let  event_json = serde_json::json!({
                "time":SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
                "self_id":self.self_id,
                "post_type":"notice",
                "notice_type":"group_increase",
                "sub_type":"approve",
                "message_id":0,
                "group_id":it.group_id,
                "operator_id":user_id,
                "user_id":user_id,
            });
            self.send_to_onebot_client(&event_json.to_string()).await;
        }
        Ok(())
    }
    async fn deal_group_event(&self,data:&serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let user_id_str = data.get("author_id").ok_or("author_id not found")?.as_str().ok_or("author_id not str")?;
        let user_id = user_id_str.parse::<u64>()?;
        if user_id == 1 { // 系统消息
            let tp = data.get("type").ok_or("type not found")?.as_i64().ok_or("type not i64")?;
            if tp != 255 {
                return Ok(()); // 不是系统消息，直接返回
            }
            let sub_type = data.get("extra").ok_or("extra not found")?.get("type").ok_or("type not found")?.as_str().ok_or("type not str")?;
            if sub_type == "exited_guild" {
                self.deal_group_decrease_event(data).await?;
            } else if sub_type == "joined_guild" {
                self.deal_group_increase_event(data).await?;
            } else if sub_type == "deleted_message" {
                self.deal_group_recall(data).await?;
            }
        } else {
            let self_id = self.self_id;
            if user_id != self_id {
                self.deal_group_message_event(data,user_id).await?;
            }
            
        }
        Ok(())
    }

    async fn deal_person_event(&self,data:&serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let user_id_str = data.get("author_id").ok_or("author_id not found")?.as_str().ok_or("author_id not str")?;
        let user_id = user_id_str.parse::<u64>()?;
        if user_id == 1 { // 系统消息
            let tp = data.get("type").ok_or("type not found")?.as_i64().ok_or("type not i64")?;
            if tp != 255 {
                return Ok(()); // 不是系统消息，直接返回
            }
            let sub_type = data.get("extra").ok_or("extra not found")?.get("type").ok_or("type not found")?.as_str().ok_or("type not str")?;
            if sub_type == "self_exited_guild" {
                // self.deal_group_kick_me_event(data).await?;
            } else if sub_type == "deleted_private_message" {
                self.deal_private_recall(data).await?;
            }
        } else {
            self.deal_private_message_event(data,user_id).await?;
        }
        Ok(())
    }
    async fn deal_kook_event(&self,data:serde_json::Value)-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tp = data.get("channel_type").ok_or("channel_type not found")?.as_str().ok_or("channel_type not str")?;
        if tp == "GROUP" {
            self.deal_group_event(&data).await?;
        }else if tp == "PERSON" {
            self.deal_person_event(&data).await?;
        }
        Ok(())
    }

    async fn deal_ob_send_group_msg(&self,params:&serde_json::Value,_js:&serde_json::Value,echo:&str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let group_id = get_json_str(params,"group_id");
        let message_arr:serde_json::Value;
        let message_rst = params.get("message").ok_or("message not found")?;
        
        if message_rst.is_string() {
            message_arr = str_msg_to_arr(message_rst)?;
        }else {
            message_arr = params.get("message").ok_or("message not found")?.to_owned();
        }
        
        let mut to_send_data: Vec<(i32, String)> = vec![];
        let mut last_type = 1;
        for it in message_arr.as_array().ok_or("message not arr")? {
            let tp = it.get("type").ok_or("type not found")?;
            if tp == "text"{
                let t = it.get("data").ok_or("data not found")?.get("text").ok_or("text not found")?.as_str().ok_or("text not str")?.to_owned();
                let s = make_kook_text(&t);
                if last_type == 1 && to_send_data.len() != 0 {
                    let l = to_send_data.len();
                    to_send_data.get_mut(l - 1).unwrap().1.push_str(&s);
                } else {
                    to_send_data.push((1,s));
                    last_type = 1;
                }
            }else if tp == "image"{
                let file = it.get("data").ok_or("data not found")?.get("file").ok_or("file not found")?.as_str().ok_or("file not str")?;
                let file_url = self.upload_image(file).await?;
                to_send_data.push((2,file_url));
                last_type = 2;
            }
            else if tp == "at"{
                let qq = it.get("data").ok_or("data not found")?.get("qq").ok_or("qq not found")?.as_str().ok_or("qq not str")?;
                let at_str = &format!("(met){}(met)",qq);
                if last_type == 1 && to_send_data.len() != 0 {
                    let l = to_send_data.len();
                    to_send_data.get_mut(l - 1).unwrap().1.push_str(at_str);
                } else {
                    to_send_data.push((1,at_str.to_owned()));
                    last_type = 1;
                }
            }
            else {
                let j = serde_json::json!([it]);
                let s = arr_to_cq_str(&j)?;
                let s2 = make_kook_text(&s);
                if last_type == 1 && to_send_data.len() != 0 {
                    let l = to_send_data.len();
                    to_send_data.get_mut(l - 1).unwrap().1.push_str(&s2);
                } else {
                    to_send_data.push((1,s2));
                    last_type = 1;
                }
            }
        }
        let mut msg_ids = vec![];
        for (tp,msg) in & to_send_data.clone() {
            let msg_id = self.send_group_msg(*tp,&group_id,msg).await?;
            msg_ids.push(msg_id);
        }
        let msg_id = crate::msgid_tool::add_msg_id(QMessageStruct{ raw_ids: msg_ids, user_id: self.self_id });
        let send_json = serde_json::json!({
            "status":"ok",
            "retcode":0,
            "data": {
                "message_id":msg_id
            },
            "echo":echo
        });
        Ok(send_json)
    }


    async fn deal_ob_send_private_msg(&self,params:&serde_json::Value,_js:&serde_json::Value,echo:&str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let user_id = get_json_str(params,"user_id");
        let message_arr:serde_json::Value;
        let message_rst = params.get("message").ok_or("message not found")?;
        
        if message_rst.is_string() {
            message_arr = str_msg_to_arr(message_rst)?;
        }else {
            message_arr = params.get("message").ok_or("message not found")?.to_owned();
        }
        
        let mut to_send_data: Vec<(i32, String)> = vec![];
        let mut last_type = 1;
        for it in message_arr.as_array().ok_or("message not arr")? {
            let tp = it.get("type").ok_or("type not found")?;
            if tp == "text"{
                let t = it.get("data").ok_or("data not found")?.get("text").ok_or("text not found")?.as_str().ok_or("text not str")?.to_owned();
                let s = make_kook_text(&t);
                if last_type == 1 && to_send_data.len() != 0 {
                    let l = to_send_data.len();
                    to_send_data.get_mut(l - 1).unwrap().1.push_str(&s);
                } else {
                    to_send_data.push((1,s));
                    last_type = 1;
                }
            }else if tp == "image"{
                let file = it.get("data").ok_or("data not found")?.get("file").ok_or("file not found")?.as_str().ok_or("file not str")?;
                let file_url = self.upload_image(file).await?;
                to_send_data.push((2,file_url));
                last_type = 2;
            }
            else if tp == "at"{
                // 私聊不支持at
            }
            else {
                let j = serde_json::json!([it]);
                let s = arr_to_cq_str(&j)?;
                let s2 = make_kook_text(&s);
                if last_type == 1 && to_send_data.len() != 0 {
                    let l = to_send_data.len();
                    to_send_data.get_mut(l - 1).unwrap().1.push_str(&s2);
                } else {
                    to_send_data.push((1,s2));
                    last_type = 1;
                }
            }
        }
        let mut msg_ids = vec![];
        for (tp,msg) in & to_send_data.clone() {
            let msg_id = self.send_private_msg(*tp,&user_id,msg).await?;
            msg_ids.push(msg_id);
        }
        let msg_id = crate::msgid_tool::add_msg_id(QMessageStruct{ raw_ids: msg_ids, user_id: self.self_id });
        let send_json = serde_json::json!({
            "status":"ok",
            "retcode":0,
            "data": {
                "message_id":msg_id
            },
            "echo":echo
        });
        Ok(send_json)
    }

    async fn deal_ob_get_login_info(&self,_params:&serde_json::Value,_js:&serde_json::Value,echo:&str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let info: LoginInfo = self.get_login_info().await?;
        let send_json = serde_json::json!({
            "status":"ok",
            "retcode":0,
            "data": info,
            "echo":echo
        });
        Ok(send_json)
    }

    async fn deal_ob_get_stranger_info(&self,params:&serde_json::Value,_js:&serde_json::Value,echo:&str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let user_id = get_json_str(params,"user_id");
        let info = self.get_stranger_info(&user_id).await?;
        let send_json = serde_json::json!({
            "status":"ok",
            "retcode":0,
            "data": info,
            "echo":echo
        });
        Ok(send_json)
    }

    async fn deal_ob_get_group_info(&self,params:&serde_json::Value,_js:&serde_json::Value,echo:&str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let group_id = get_json_str(params,"group_id");
        let info = self.get_group_info(&group_id).await?;
        let send_json = serde_json::json!({
            "status":"ok",
            "retcode":0,
            "data": info,
            "echo":echo
        });
        Ok(send_json)
    }

    async fn deal_ob_get_group_list(&self,_params:&serde_json::Value,_js:&serde_json::Value,echo:&str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let info = self.get_group_list().await?;
        let send_json = serde_json::json!({
            "status":"ok",
            "retcode":0,
            "data": info,
            "echo":echo
        });
        Ok(send_json)
    }

    
    async fn deal_ob_get_group_member_info(&self,params:&serde_json::Value,_js:&serde_json::Value,echo:&str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let group_id = get_json_str(params,"group_id");
        let user_id = get_json_str(params,"user_id");
        let info = self.get_group_member_info(&group_id, &user_id).await?;
        let send_json = serde_json::json!({
            "status":"ok",
            "retcode":0,
            "data": info,
            "echo":echo
        });
        Ok(send_json)
    }

    async fn deal_ob_set_group_kick(&self,params:&serde_json::Value,_js:&serde_json::Value,echo:&str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let group_id = get_json_str(params,"group_id");
        let user_id = get_json_str(params,"user_id");
        self.set_group_kick(&group_id, &user_id).await?;
        let send_json = serde_json::json!({
            "status":"ok",
            "retcode":0,
            "data": {},
            "echo":echo
        });
        Ok(send_json)
    }

    async fn deal_ob_delete_msg(&self,params:&serde_json::Value,_js:&serde_json::Value,echo:&str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let msg_id = get_json_str(params,"message_id").parse::<i32>()?;
        let msg_ids = crate::msgid_tool::get_msg_id(msg_id);
        for it in msg_ids.raw_ids {
            self.delete_msg(&it).await?;
        }
        let send_json = serde_json::json!({
            "status":"ok",
            "retcode":0,
            "data": {},
            "echo":echo
        });
        Ok(send_json)
    }

    async fn deal_ob_set_group_leave(&self,params:&serde_json::Value,_js:&serde_json::Value,echo:&str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let group_id = get_json_str(params,"group_id");
        self.set_group_leave(&group_id).await?;
        let send_json = serde_json::json!({
            "status":"ok",
            "retcode":0,
            "data": {},
            "echo":echo
        });
        Ok(send_json)
    }

    async fn deal_ob_set_group_name(&self,params:&serde_json::Value,_js:&serde_json::Value,echo:&str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let group_id = get_json_str(params,"group_id");
        let group_name = get_json_str(params,"group_name");
        self.set_group_name(&group_id,&group_name).await?;
        let send_json = serde_json::json!({
            "status":"ok",
            "retcode":0,
            "data": {},
            "echo":echo
        });
        Ok(send_json)
    }

    async fn deal_ob_set_group_card(&self,params:&serde_json::Value,_js:&serde_json::Value,echo:&str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let group_id = get_json_str(params,"group_id");
        let user_id = get_json_str(params,"user_id");
        let card = get_json_str(params,"card");
        self.set_group_card(&group_id,&user_id,&card).await?;
        let send_json = serde_json::json!({
            "status":"ok",
            "retcode":0,
            "data": {},
            "echo":echo
        });
        Ok(send_json)
    }

    async fn deal_ob_get_friend_list(&self,_params:&serde_json::Value,_js:&serde_json::Value,echo:&str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let info = self.get_friend_list().await?;
        let send_json = serde_json::json!({
            "status":"ok",
            "retcode":0,
            "data": info,
            "echo":echo
        });
        Ok(send_json)
    }

    // 这个函数处理onebot的api调用
    pub async fn deal_onebot(&self,_uid:&str,text:&str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let js:serde_json::Value = serde_json::from_str(&text)?;
        let action = js.get("action").ok_or("action not found")?.as_str().ok_or("action not str")?;
        let def = serde_json::json!({});
        let params = js.get("params").unwrap_or(&def);
        let echo = get_json_str(&js,"echo");
        let send_json;
        println!("收到来自onebot的动作:{text}");
        send_json = match action {
            "send_group_msg" => {
                self.deal_ob_send_group_msg(&params,&js,&echo).await?
            },
            "send_private_msg" => {
                self.deal_ob_send_private_msg(&params,&js,&echo).await?
            },
            "send_msg" => {
                let group_id = get_json_str(params, "group_id");
                if group_id != "" {
                    self.deal_ob_send_group_msg(&params,&js,&echo).await?
                }else {
                    self.deal_ob_send_private_msg(&params,&js,&echo).await?
                }
            },
            "get_login_info" => {
                self.deal_ob_get_login_info(&params,&js,&echo).await?
            },
            "get_stranger_info" => {
                self.deal_ob_get_stranger_info(&params,&js,&echo).await?
            },
            "get_group_info" => {
                self.deal_ob_get_group_info(&params,&js,&echo).await?
            },
            "get_group_list" => {
                self.deal_ob_get_group_list(&params,&js,&echo).await?
            },
            "get_group_member_info" => {
                self.deal_ob_get_group_member_info(&params,&js,&echo).await?
            },
            "set_group_kick" => {
                self.deal_ob_set_group_kick(&params,&js,&echo).await?
            },
            "delete_msg" => {
                self.deal_ob_delete_msg(&params,&js,&echo).await?
            },
            "set_group_leave" => {
                self.deal_ob_set_group_leave(&params,&js,&echo).await?
            }
            "set_group_name" => {
                self.deal_ob_set_group_name(&params,&js,&echo).await?
            },
            "set_group_card" => {
                self.deal_ob_set_group_card(&params,&js,&echo).await?
            }
            "get_friend_list" => {
                self.deal_ob_get_friend_list(&params,&js,&echo).await?
            }
            "can_send_image" => {
                serde_json::json!({
                    "status":"ok",
                    "retcode":0,
                    "data": {"yes":true},
                    "echo":echo
                })
            },
            "can_send_record" => {
                serde_json::json!({
                    "status":"ok",
                    "retcode":0,
                    "data": {"yes":false},
                    "echo":echo
                })
            },
            "get_status" => {
                serde_json::json!({
                    "status":"ok",
                    "retcode":0,
                    "data": {
                        "online":true,
                        "good":true
                    },
                    "echo":echo
                })
            },
            "get_version_info" => {
                serde_json::json!({
                    "status":"ok",
                    "retcode":0,
                    "data": {
                        "app_name":"kook-onebot",
                        "app_version":"0.0.4",
                        "protocol_version":"v11"
                    },
                    "echo":echo
                })
            },
            _ => {
                serde_json::json!({
                    "status":"failed",
                    "retcode":-1,
                    "data": {},
                    "echo":echo
                })
            }
        };
        let json_str = send_json.to_string();
        println!("ONEBOT动作返回:{}",json_str);
        Ok(json_str)
    }
}


fn get_json_str(js:&serde_json::Value,key:&str) -> String {
    let key_val = js.get(key);
    if key_val.is_none() {
        return "".to_owned();
    }
    let val = key_val.unwrap();
    if val.is_i64() {
        return val.as_i64().unwrap().to_string();
    }
    if val.is_u64() {
        return val.as_u64().unwrap().to_string();
    }
    if val.is_string() {
        return val.as_str().unwrap().to_string();
    }
    return "".to_owned();
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug)]
struct GroupInfo {
    group_id:u64,
    group_name:String,
    member_count:i32,
    max_member_count:i32
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct LoginInfo {
    pub user_id:u64,
    pub nickname:String
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug)]
struct StrangerInfo {
    user_id:u64,
    nickname:String,
    sex:String,
    age:i32
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug)]
pub struct FriendInfo {
    user_id:u64,
    nickname:String,
    remark:String,
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug,Clone)]
struct GroupMemberInfo {
    group_id:u64,
    user_id:u64,
    nickname:String,
    card:String,
    sex:String,
    age:i32,
    area:String,
    join_time:i32,
    last_sent_time:i32,
    level:String,
    role:String,
    unfriendly:bool,
    title:String,
    title_expire_time:i32,
    card_changeable:bool
}