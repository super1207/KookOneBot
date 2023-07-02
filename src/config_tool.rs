
use tokio::sync::RwLock;

fn deal_path_str(path_str:&str) -> &str {
    if path_str.starts_with("\\\\?\\") {
        return &path_str[4..];
    }else{
        return path_str;
    }
}

fn get_run_dir() -> Result<String, Box<dyn std::error::Error>>{
    let exe_dir = std::env::current_exe()?;
    let exe_path = exe_dir.parent().ok_or("无法获得运行目录")?;
    let mut exe_path_str = exe_path.to_string_lossy().to_string();
    if !exe_path_str.ends_with(std::path::MAIN_SEPARATOR)
    {
        exe_path_str.push(std::path::MAIN_SEPARATOR);
    }
    return Ok(deal_path_str(&exe_path_str).to_string());
}

pub async fn read_config() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    lazy_static! {
        static ref JS_VAL: RwLock<Option<serde_json::Value>> = RwLock::new(None);
    }

    {
        let lk = JS_VAL.read().await;
        if lk.is_some() {
            return Ok(lk.clone().unwrap().clone());
        }
    }

    let run_dir = get_run_dir()?;
    let config_file_dir = run_dir + "config.json";

    let mut is_file_exists = false;
    if let Ok(metadata) = tokio::fs::metadata(config_file_dir.clone()).await {
        if metadata.is_file() {
            is_file_exists = true;
        }
    }
    if !is_file_exists{
        tokio::fs::write(config_file_dir.clone(), "{\"web_port\":8080,\"kook_token\":\"\",\"access_token\":\"\",\"web_host\":\"127.0.0.1\",\"reverse_uri\":[]}").await?;
        log::error!("config.json文件不存在，为您自动生成！请自行修改后重新运行！！！");
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    }
    
    
    let file_str = tokio::fs::read_to_string(config_file_dir).await?;
    let json_val:serde_json::Value = serde_json::from_str(&file_str)?;
    {
        let mut lk = JS_VAL.write().await;
        (*lk) = Some(json_val.clone())
    }
    Ok(json_val)
}