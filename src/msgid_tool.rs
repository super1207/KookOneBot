use std::collections::HashMap;


lazy_static! {
    static ref MSG_ID_LIST : std::sync::RwLock<HashMap<i32,Vec<String>>>  = std::sync::RwLock::new(HashMap::new());
}


fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFFu32;
    let table = generate_crc32_table();

    for byte in data.iter() {
        let index = ((crc ^ u32::from(*byte)) & 0xFF) as usize;
        crc = (crc >> 8) ^ table[index];
    }

    !crc
}

fn generate_crc32_table() -> [u32; 256] {
    const POLY: u32 = 0xEDB88320;

    let mut table = [0u32; 256];

    for i in 0..256 {
        let mut crc = i as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = POLY ^ (crc >> 1);
            } else {
                crc >>= 1;
            }
        }
        table[i] = crc;
    }

    table
}



pub fn add_msg_id(msg_ids:Vec<String>) -> i32 {
    if msg_ids.len() == 0 {
        return 0;
    }
    let msg0 = msg_ids.get(0).unwrap();
    let crc_num = crc32(msg0.as_bytes()) as i32;
    let mut lk = MSG_ID_LIST.write().unwrap();
    if lk.len() > 9999999 {
        lk.clear();
    }
    lk.insert(crc_num,msg_ids);
    crc_num
}

pub fn get_msg_id(crc_num:i32) -> Vec<String> {
    let lk = MSG_ID_LIST.read().unwrap();
    if let Some(v) = lk.get(&crc_num) {
        v.to_owned()
    }else {
        vec![]
    }
}