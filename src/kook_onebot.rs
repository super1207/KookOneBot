

use std::{str::FromStr, io::Read, collections::HashMap};

use flate2::read::ZlibDecoder;
use futures_util::StreamExt;
use hyper::http::{HeaderName, HeaderValue};
use serde_derive::{Serialize, Deserialize};
use tokio_tungstenite::connect_async;
use std::time::SystemTime;

use crate::{G_ONEBOT_RX, cqtool::{str_msg_to_arr, arr_to_cq_str, cq_params_encode}};

pub(crate) struct KookOnebot {
    pub token:String,
    pub self_id:i64
}

impl KookOnebot {

    async fn http_get_json(&self,uri:&str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>{
        let uri = reqwest::Url::from_str(&format!("https://www.kookapp.cn/api/v3{uri}"))?;
        let client = reqwest::Client::builder().danger_accept_invalid_certs(true).no_proxy().build()?;
        let mut req = client.get(uri).build()?;
        let token = &self.token;
        req.headers_mut().append(HeaderName::from_str("Authorization")?, HeaderValue::from_str(&format!("Bot {token}"))?);
        let ret = client.execute(req).await?;
        let retbin = ret.bytes().await?.to_vec();
        let ret_str = String::from_utf8(retbin)?;
        let js:serde_json::Value = serde_json::from_str(&ret_str)?;
        let ret = js.get("data").ok_or("get data err")?;
        Ok(ret.to_owned())
    }

    async fn http_post_json(&self,uri:&str,json:&serde_json::Value) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>{
        let uri = reqwest::Url::from_str(&format!("https://www.kookapp.cn/api/v3{uri}"))?;
        let client = reqwest::Client::builder().danger_accept_invalid_certs(true).no_proxy().build()?;
        let mut req = client.post(uri).body(reqwest::Body::from(json.to_string())).build()?;
        let token = &self.token;
        req.headers_mut().append(HeaderName::from_str("Authorization")?, HeaderValue::from_str(&format!("Bot {token}"))?);
        req.headers_mut().append(HeaderName::from_str("Content-type")?, HeaderValue::from_str("application/json")?);
        let ret = client.execute(req).await?;
        let retbin = ret.bytes().await?.to_vec();
        let ret_str = String::from_utf8(retbin)?;
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
                        group_id:id.parse::<i64>()?,
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
    pub async fn get_login_info(&self)-> Result<LoginInfo, Box<dyn std::error::Error + Send + Sync>> {
        let login_info = self.http_get_json("/user/me").await?;
        let user_id = login_info.get("id").ok_or("get id err")?.as_str().ok_or("id not str")?;
        let nickname = login_info.get("username").ok_or("get username err")?.as_str().ok_or("username not str")?;
        Ok(LoginInfo {
            user_id:user_id.parse::<i64>()?,
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
    pub async fn upload_image(&self,url:&str)-> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let ret_bin = Self::http_post(url,vec![],&HashMap::new(),false).await?;
        let uri = reqwest::Url::from_str(&format!("https://www.kookapp.cn/api/v3/asset/create"))?;
        let client = reqwest::Client::builder().danger_accept_invalid_certs(true).no_proxy().build()?;
        let form = reqwest::multipart::Form::new().part("file", reqwest::multipart::Part::bytes(ret_bin).file_name("test"));
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
            user_id:user_id.parse::<i64>()?,
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
            group_id:group_id.parse::<i64>()?,
            group_name:group_name.to_owned(),
            member_count:0,
            max_member_count:0
        })
    }

    #[allow(dead_code)]
    async fn get_group_member_info(&self,group_id:&str,user_id:&str)-> Result<GroupMemberInfo, Box<dyn std::error::Error + Send + Sync>> {
        let group_info = self.http_get_json(&format!("/channel/view?target_id={group_id}")).await?;
        let guild_id = group_info.get("guild_id").ok_or("get guild_id err")?.as_str().ok_or("guild_id not str")?;
        let stranger_info = self.http_get_json(&format!("/user/view?user_id={user_id}&guild_id={guild_id}")).await?;
        Ok(GroupMemberInfo {
            group_id:group_id.parse::<i64>()?,
            user_id:user_id.parse::<i64>()?,
            nickname:stranger_info.get("username").ok_or("get username err")?.as_str().ok_or("username not str")?.to_owned(),
            card:stranger_info.get("nickname").ok_or("get nickname err")?.as_str().ok_or("nickname not str")?.to_owned(),
            sex:"unknown".to_owned(),
            age:0,
            area:"".to_owned(),
            join_time:(stranger_info.get("joined_at").ok_or("get joined_at err")?.as_u64().ok_or("joined_at not u64")? / 1000) as i32,
            last_sent_time:(stranger_info.get("active_time").ok_or("get active_time err")?.as_u64().ok_or("active_time not u64")? / 1000) as i32,
            level:"".to_owned(),
            role:"member".to_owned(),
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

    #[allow(dead_code)]
    async fn send_group_msg(&self,tp:i32,group_id:&str,message:&str)-> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut json:serde_json::Value = serde_json::from_str("{}")?;
        json["content"] = message.into();
        json["target_id"] = group_id.into();
        json["type"] = tp.into();
        let _ret_json = self.http_post_json("/message/create",&json).await?;
        Ok("0".to_string())
    }


    async fn get_gateway(&self)-> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let ret_json = self.http_get_json(&format!("/gateway/index?compress=1")).await?;
        Ok(ret_json.get("url").ok_or("get url err")?.as_str().ok_or("url not str")?.to_owned())
    }

    pub async fn connect(&self)-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let wss_url = self.get_gateway().await?;
        println!("connect to:{wss_url}");
        let (ws_stream, _) = connect_async(wss_url).await?;
        let (_,mut read_halt) = ws_stream.split();
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
                    println!("reconnect");
                    break;
                }
                else if s == 1 {
                    println!("connect ok");
                }
                else if s == 0 {
                    println!("recv event:{}",js.to_string());
                    let d = js.get("d").ok_or("d not found")?;
                    let rst = self.deal_kook_event(d.clone()).await;
                    if rst.is_err() {
                        println!("{rst:?}");
                    }
                }
            }
        }
        Ok(())
    }
    pub async fn get_lifecycle_event(&self)-> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let self_id = crate::G_SELF_ID.read().await;
        let tm = SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs().to_string();
        let ret = format!("{{\"meta_event_type\":\"lifecycle\",\"post_type\":\"meta_event\",\"self_id\":{self_id},\"sub_type\":\"connect\",\"time\":{tm}}}");
        Ok(ret)
    }
    async fn deal_kook_event(&self,data:serde_json::Value)-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tp = data.get("channel_type").ok_or("channel_type not found")?.as_str().ok_or("channel_type not str")?;
        if tp == "GROUP" {
            let user_id_str = data.get("author_id").ok_or("author_id not found")?.as_str().ok_or("author_id not str")?;
            let user_id = user_id_str.parse::<i64>()?;
            let group_id_str = data.get("target_id").ok_or("target_id not found")?.as_str().ok_or("target_id not str")?;
            let group_id = group_id_str.parse::<i64>()?;
            let mut message = data.get("content").ok_or("content not found")?.as_str().ok_or("content not str")?.to_owned();
            let sender = self.get_group_member_info(group_id_str,user_id_str).await?;
            // let message_id_str = data.get("msg_id").ok_or("msg_id not found")?.as_str().ok_or("msg_id not str")?;
            let msg_type = data.get("type").ok_or("type not found")?.as_i64().ok_or("type not i64")?;
            if msg_type == 2 {
                message = format!("[CQ:image,file={}]",cq_params_encode(&message));
            }
            let  event_json = serde_json::json!({
                "time":SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs().to_string(),
                "self_id":crate::G_SELF_ID.read().await.to_owned(),
                "post_type":"message",
                "message_type":"group",
                "sub_type":"normal",
                "message_id":0,
                "group_id":group_id,
                "user_id":user_id,
                "message":message,
                "raw_message":message,
                "font":0,
                "sender":sender
            });
            let lk = G_ONEBOT_RX.read().await;
            for (_,v) in &*lk {
                v.send(event_json.to_string()).await?;
            }
        }
        Ok(())
    }
    // 这个函数处理onebot的api调用
    pub async fn deal_onebot(&self,uid:&str,text:&str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        fn get_json_str(js:&serde_json::Value,key:&str) -> String {
            let key_val = js.get(key);
            if key_val.is_none() {
                return "".to_owned();
            }
            let val = key_val.unwrap();
            if val.is_i64() {
                return val.as_i64().unwrap().to_string();
            }
            if val.is_string() {
                return val.as_str().unwrap().to_string();
            }
            return "".to_owned();
        }
        let js:serde_json::Value = serde_json::from_str(&text)?;
        let action = js.get("action").ok_or("action not found")?.as_str().ok_or("action not str")?;
        let def = serde_json::json!({});
        let params = js.get("params").unwrap_or(&def);
        if action == "send_group_msg" {
            let group_id = get_json_str(params,"group_id");
            let message_arr:serde_json::Value;
            let message_rst = params.get("message").ok_or("message not found")?;
            
            if message_rst.is_string() {
                message_arr = str_msg_to_arr(message_rst)?;
            }else {
                message_arr = params.get("message").ok_or("message not found")?.to_owned();
            }
           
            let mut to_send_data = vec![];
            for it in message_arr.as_array().ok_or("message not arr")? {
                let tp = it.get("type").ok_or("type not found")?;
                if tp == "text"{
                    let t = it.get("data").ok_or("data not found")?.get("text").ok_or("text not found")?.as_str().ok_or("text not str")?;
                    to_send_data.push((1,t.to_owned()))
                }else if tp == "image"{
                    let file = it.get("data").ok_or("data not found")?.get("file").ok_or("file not found")?.as_str().ok_or("file not str")?;
                    let file_url = self.upload_image(file).await?;
                    to_send_data.push((2,file_url));
                }
                else {
                    let j = serde_json::json!(it);
                    let s = arr_to_cq_str(&j)?;
                    to_send_data.push((1,s)) 
                }
            }
            let echo = get_json_str(&js,"echo");
            for (tp,msg) in & to_send_data.clone() {
                self.send_group_msg(*tp,&group_id,msg).await?;
            }
            let send_json = serde_json::json!({
                "status":"ok",
                "retcode":0,
                "data": {
                    "message_id":0
                },
                "echo":echo
            });
            let lk = G_ONEBOT_RX.read().await;
            if let Some(tx) = lk.get(uid) {
                tx.send(send_json.to_string()).await?;
            }
        }
        Ok("".to_owned())
    }
}


#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug)]
struct GroupInfo {
    group_id:i64,
    group_name:String,
    member_count:i32,
    max_member_count:i32
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct LoginInfo {
    pub user_id:i64,
    pub nickname:String
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug)]
struct StrangerInfo {
    user_id:i64,
    nickname:String,
    sex:String,
    age:i32
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug)]
struct GroupMemberInfo {
    group_id:i64,
    user_id:i64,
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