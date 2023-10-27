use std::{collections::HashMap, sync::{atomic::AtomicI64, Arc}};

use crate::{G_KOOK_TOKEN, G_SELF_ID, G_ACCESS_TOKEN};


pub fn get_params_from_uri(uri:&hyper::Uri) -> HashMap<String,String> {
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


pub async fn deal_onebot_http(mut request: hyper::Request<hyper::Body>) -> Result<hyper::Response<hyper::Body>, Box<dyn std::error::Error + Send + Sync>> {
    let url_path = request.uri().path().to_owned();
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
        let headers_map = request.headers();
        if let Some(content_type) = headers_map.get("content-type") {
            if content_type.to_str()? == "application/json" {
                let body = hyper::body::to_bytes(request.body_mut()).await?;
                match serde_json::from_slice(&body) {
                    Ok(v) => {
                        params = v;
                    } ,
                    Err(_) => {
                        let ret = serde_json::json!({
                            "status":"failed",
                            "retcode":1400,
                        });
                        log::error!("ONEBOT动作调用出错:`INVALID JSON`");
                        let mut res = hyper::Response::new(hyper::Body::from(ret.to_string()));
                        (*res.status_mut()) = hyper::StatusCode::BAD_REQUEST;
                        res.headers_mut().insert("Content-Type", hyper::http::HeaderValue::from_static("application/json"));
                        return Ok(res);
                    },
                }
            } else if content_type.to_str()? == "application/x-www-form-urlencoded" {
                let body = hyper::body::to_bytes(request.body_mut()).await?;
                params = url::form_urlencoded::parse(&body).collect::<serde_json::Value>();
            } else {
                let ret = serde_json::json!({
                    "status":"failed",
                    "retcode":1406,
                });
                log::error!("ONEBOT动作调用出错:`HTTP 406`");
                let mut res = hyper::Response::new(hyper::Body::from(ret.to_string()));
                (*res.status_mut()) = hyper::StatusCode::NOT_ACCEPTABLE;
                res.headers_mut().insert("Content-Type", hyper::http::HeaderValue::from_static("application/json"));
                return Ok(res);
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
    let (http_code,ret) = kb.deal_onebot(&js.to_string()).await;
    let mut res = hyper::Response::new(hyper::Body::from(ret));
    if http_code == 404 {
        (*res.status_mut()) = hyper::StatusCode::NOT_FOUND;
    }
    res.headers_mut().insert("Content-Type", hyper::http::HeaderValue::from_static("application/json"));
    return Ok(res);
}


pub async fn check_auth(request: &hyper::Request<hyper::Body>) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    // 获得当前的访问密钥
    let mut is_pass = false;
    let g_access_token = G_ACCESS_TOKEN.read().await.clone();
    let headers_map = request.headers();
    if !g_access_token.is_empty() {
        // 两个地方任何有一个满足要求，则通过
        {
            let access_token:String; 
            if let Some(token) = headers_map.get("Authorization") {
                access_token = token.to_str()?.to_owned();
            }
            else {
                access_token = "".to_owned();
            }
            if access_token == "Bearer ".to_owned() + &g_access_token {
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
    Ok(is_pass)
}