use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about = "云拼音命令行工具", long_about = None)]
struct Args {
    #[arg(short, long, default_value = "sougou", help = "云拼音引擎: baidu, google, sougou, custom")]
    engine: String,

    #[arg(short, long, help = "输出格式: simple (仅词汇), tsv (词\\长度\\拼音)", default_value = "simple")]
    format: String,

    #[arg(long, help = "自定义 API URL 模板, 用 {input} 作为拼音占位符 (需 -e custom)")]
    api_url: Option<String>,

    #[arg(help = "输入的拼音")]
    input: String,
}

fn main() {
    let args = Args::parse();

    let result = match args.engine.as_str() {
        "baidu" => fetch_baidu(&args.input),
        "google" => fetch_google(&args.input),
        "sougou" => fetch_sougou(&args.input),
        "custom" => {
            let url = args.api_url.as_deref().unwrap_or_else(|| {
                eprintln!("自定义引擎需要指定 --api-url");
                std::process::exit(1);
            });
            fetch_custom(url, &args.input)
        }
        _ => {
            eprintln!("未知的引擎: {}", args.engine);
            std::process::exit(1);
        }
    };

    match result {
        Ok(words) => {
            for word in words {
                match args.format.as_str() {
                    "tsv" => println!("{}\t{}\t{}", word.text, word.length, word.preedit),
                    _ => println!("{}", word.text),
                }
            }
        }
        Err(e) => {
            eprintln!("请求失败: {}", e);
            std::process::exit(1);
        }
    }
}

#[derive(Debug)]
struct Word {
    text: String,
    length: usize,
    preedit: String,
}

fn fetch_baidu(input: &str) -> Result<Vec<Word>, Box<dyn std::error::Error>> {
    let url = format!(
        "https://olime.baidu.com/py?input={}&inputtype=py&bg=0&ed=5&result=hanzi&resultcoding=utf-8&ch_en=0&clientinfo=web&version=1",
        urlencoding::encode(input)
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;
    
    let resp = client.get(&url).send()?;
    let text = resp.text()?;
    
    // 调试输出
    if std::env::var("DEBUG").is_ok() {
        eprintln!("[baidu] 响应: {}", text);
    }
    
    let json: serde_json::Value = serde_json::from_str(&text)?;
    let mut words = Vec::new();

    if json["status"] == "T" {
        if let Some(result) = json["result"].as_array() {
            if let Some(first) = result.get(0).and_then(|r| r.as_array()) {
                for item in first.iter().take(5) {
                    if let Some(item_arr) = item.as_array() {
                        if item_arr.len() >= 3 {
                            let text = item_arr[0].as_str().unwrap_or("").to_string();
                            let length = item_arr[1].as_u64().unwrap_or(0) as usize;
                            let pinyin_info = &item_arr[2];
                            let pinyin = pinyin_info["pinyin"].as_str().unwrap_or("").to_string();
                            let preedit = pinyin.replace("'", " ");
                            
                            words.push(Word {
                                text,
                                length,
                                preedit,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(words)
}

fn fetch_google(input: &str) -> Result<Vec<Word>, Box<dyn std::error::Error>> {
    let url = format!(
        "https://inputtools.google.com/request?text={}&itc=zh-t-i0-pinyin&num=5&cp=0&cs=1&ie=utf-8&oe=utf-8",
        urlencoding::encode(input)
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(1500))
        .build()?;
    
    let resp = client.get(&url).send()?;
    let text = resp.text()?;
    
    let json: serde_json::Value = serde_json::from_str(&text)?;
    let mut words = Vec::new();

    if json[0] == "SUCCESS" {
        if let Some(results) = json[1].as_array() {
            if let Some(first) = results.get(0).and_then(|r| r.as_array()) {
                let input_text = first[0].as_str().unwrap_or("");
                let candidates = first[1].as_array();
                let meta = first.get(3);
                
                if let Some(candidates) = candidates {
                    for (i, candidate) in candidates.iter().take(5).enumerate() {
                        let text = candidate.as_str().unwrap_or("").to_string();
                        
                        // 获取匹配长度
                        let length = meta
                            .and_then(|m| m["matched_length"].as_array())
                            .and_then(|arr| arr.get(i))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(input_text.len() as u64) as usize;
                        
                        // 获取拼音注释
                        let preedit = meta
                            .and_then(|m| m["annotation"].as_array())
                            .and_then(|arr| arr.get(i))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        
                        words.push(Word {
                            text,
                            length,
                            preedit,
                        });
                    }
                }
            }
        }
    }

    Ok(words)
}

fn fetch_sougou(input: &str) -> Result<Vec<Word>, Box<dyn std::error::Error>> {
    // 序列化密钥
    let data = serial_keys(input);
    
    let url = format!(
        "http://shouji.sogou.com/web_ime/mobile.php?durtot=0&h=000000000000000&r=store_mf_wandoujia&v=3.7"
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(1000))
        .build()?;
    
    let resp = client.post(&url)
        .header("Content-Type", "application/octet-stream")
        .body(data)
        .send()?;
    
    let bytes = resp.bytes()?;
    let words = parse_sougou_result(&bytes)?;
    
    Ok(words)
}

fn serial_keys(keys: &str) -> Vec<u8> {
    let token = vec![0u8, 5, 0, 0, 0, 0, 1];
    let total_len = token.len() + keys.len() + 3;
    
    let mut data = Vec::new();
    data.push(total_len as u8);
    data.extend_from_slice(&token);
    data.push(keys.len() as u8);
    data.extend_from_slice(keys.as_bytes());
    
    // 计算校验和 (rc)
    let mut start: u8 = 0;
    for &b in &data {
        start ^= b;
    }
    data.push(start);
    
    data
}

fn parse_sougou_result(result: &[u8]) -> Result<Vec<Word>, Box<dyn std::error::Error>> {
    let mut words = Vec::new();
    
    if result.len() < 2 {
        return Ok(words);
    }
    
    let expected_len = (result[0] as usize) + 2;
    if expected_len != result.len() {
        eprintln!("[sougou] 警告: 数据包长度不匹配, 期望 {}, 实际 {}", expected_len, result.len());
    }
    
    if result.len() < 0x14 + 2 {
        return Ok(words);
    }
    
    // 读取词数量 (0x12 位置, 2字节小端)
    let num_words = u16::from_le_bytes([result[0x12], result[0x13]]) as usize;
    if num_words == 0 || num_words > 32 {
        eprintln!("[sougou] 警告: 词数量异常 {}", num_words);
        return Ok(words);
    }
    
    let mut pos: usize = 0x14;
    
    for _ in 0..num_words {
        if pos + 2 > result.len() {
            break;
        }
        
        let str_len = u16::from_le_bytes([result[pos], result[pos + 1]]) as usize;
        pos += 2;
        
        if str_len == 0 || str_len > 0xFF {
            eprintln!("[sougou] 错误: 无效的字符串长度 {}", str_len);
            continue;
        }
        
        if pos + str_len > result.len() {
            break;
        }
        
        // UTF-16LE 解码
        let utf16_bytes = &result[pos..pos + str_len];
        pos += str_len;
        
        let utf16_vec: Vec<u16> = utf16_bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        
        let text = String::from_utf16(&utf16_vec).unwrap_or_default();
        
        // 跳过未知字段
        if pos + 2 > result.len() {
            break;
        }
        let skip1 = u16::from_le_bytes([result[pos], result[pos + 1]]) as usize;
        pos += skip1 + 2;
        
        if pos + 2 > result.len() {
            break;
        }
        let skip2 = u16::from_le_bytes([result[pos], result[pos + 1]]) as usize;
        pos += skip2 + 2 + 1;
        
        words.push(Word {
            text,
            length: 0,
            preedit: String::new(),
        });
        
        if words.len() >= 5 {
            break;
        }
    }
    
    Ok(words)
}

fn fetch_custom(api_url: &str, input: &str) -> Result<Vec<Word>, Box<dyn std::error::Error>> {
    let url = api_url.replace("{input}", &urlencoding::encode(input));

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let resp = client.get(&url).send()?;
    let text = resp.text()?;

    if std::env::var("DEBUG").is_ok() {
        eprintln!("[custom] 响应: {}", text);
    }

    let json: serde_json::Value = serde_json::from_str(&text)?;
    let mut words = Vec::new();

    if let Some(candidates) = json["candidates"].as_array() {
        for item in candidates.iter().take(5) {
            let word_text = item["text"].as_str().unwrap_or("").to_string();
            if word_text.is_empty() {
                continue;
            }
            let preedit = item["preedit"].as_str().unwrap_or("").to_string();
            words.push(Word {
                text: word_text,
                length: input.len(),
                preedit,
            });
        }
    }

    Ok(words)
}
