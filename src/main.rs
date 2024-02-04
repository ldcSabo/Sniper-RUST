use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async_with_config, MaybeTlsStream, WebSocketStream};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use reqwest::{Client, header};
use serde_json::json;

const GATEWAY_URL: &str = "wss://gateway.discord.gg/?v=10&encoding=json";
static mut SEQ: i64 = 0;
static mut SESSIONID: String = String::new();
static mut SELFTOKEN: String = String::new();
static mut SNIPETOKEN: String = String::new();
static mut GUILDID: String = String::new();
static mut GUILDS: Vec<String> = Vec::new();
static mut VANITIES: Vec<String> = Vec::new();

async fn snipe(code: &str) -> Result<(), reqwest::Error>
{
    let client = Client::new();

    let data = json!({
        "code": code
    });

    let mut headers = header::HeaderMap::new();
    headers.insert(header::USER_AGENT, header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/118.0.0.0 Safari/537.36"));
    
    unsafe {
        headers.insert(header::AUTHORIZATION, header::HeaderValue::from_static(&SNIPETOKEN[1..SNIPETOKEN.len() - 1]));
    }
  
    let guildstored;

    unsafe {
        guildstored = GUILDID.clone();
    }

    let response = client
        .patch(format!("{}{}{}", "https://discord.com/api/v9/guilds/",guildstored.replace("\"", ""),"/vanity-url"))
        .headers(headers)
        .json(&data)
        .send()
        .await?;

    if response.status().is_success() {
        let json_response = response.json::<serde_json::Value>().await?;
        println!("Response JSON: {:?}", json_response);
    } else {
        println!("Request failed with status code: {}, {:?}", response.status(), response.json::<serde_json::Value>().await?);
    }

    Ok(())
}

async fn handle_payload(
    tx: Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    payload: String,
) {

    let payload: HashMap<String, serde_json::Value> = serde_json::from_str(&payload).unwrap();

    if let Some(value) = payload.get("seq") {
        if !value.is_null() {
            if let Some(stored_value) = value.as_i64() {
                println!("The sequence: {}", stored_value);
                unsafe {
                    SEQ = stored_value;
                }
            }
        }
    }

    match payload.get("op") {
        Some(op) => {
            let op = op.as_u64().unwrap();
            match op {
                0 => {

                    if payload.contains_key("t") && !payload.get("t").unwrap().is_null() && payload.get("t").unwrap().as_str() == Some("READY") {

                        for guild in payload.get("d").unwrap().as_object().unwrap().get("guilds").unwrap().as_array().unwrap() {
                            if guild.as_object().unwrap().contains_key("vanity_url_code") && !guild.get("vanity_url_code").unwrap().is_null() {
                                println!("NAME : {}\tID : {}\tVANITY : {}", guild.get("name").unwrap().as_str().unwrap(), guild.get("id").unwrap().as_str().unwrap(), guild.get("vanity_url_code").unwrap().as_str().unwrap());
                                unsafe {
                                    let guildself = guild.get("id").unwrap().as_str().unwrap().to_owned() + ":" + guild.get("vanity_url_code").unwrap().as_str().unwrap();
                                    
                                    if VANITIES.contains(&guild.get("vanity_url_code").unwrap().to_string()) {
                                        GUILDS.push(guildself);   
                                    }
                                }
                            }
                        }
                        unsafe {
                            println!("Total Guild Count : {}", GUILDS.len());
                            SESSIONID = payload.get("d").unwrap().as_object().unwrap().get("session_id").unwrap().as_str().unwrap().to_owned();
                            println!("Session ID : {}", SESSIONID);
                        }

                        //println!("Packet: {:?}", payload.get("d").unwrap().as_object());
                    }

                    if payload.contains_key("t") && !payload.get("t").unwrap().is_null() && payload.get("t").unwrap().as_str() == Some("GUILD_UPDATE") {

                        if payload.get("d").unwrap().as_object().unwrap().contains_key("vanity_url_code") {

                            let mut changed = false;

                            if payload.get("d").unwrap().as_object().unwrap().get("vanity_url_code").is_none() || payload.get("d").unwrap().as_object().unwrap().get("vanity_url_code").unwrap().is_null() {
                                changed = true;
                            }
                            else {
                                unsafe {
                                    for guild in GUILDS.to_vec() {
                                        if guild.contains(&payload.get("d").unwrap().as_object().unwrap().get("id").unwrap().to_string().replace("\"", "")) {
                                            if let Some(last_element) = guild.split(":").last() {
                                                if !last_element.to_owned().eq(&payload.get("d").unwrap().as_object().unwrap().get("vanity_url_code").unwrap().to_string().replace("\"", "")) {
                                                    changed = true;
                                                    break;
                                                }
                                            } else {
                                                println!("Guild has invalid data.");
                                            }
                                        }
                                    }
                                }
                            }

                            if changed {
                                unsafe {
                                    let id: String = payload.get("d").unwrap().as_object().unwrap().get("id").unwrap().as_str().unwrap().to_owned();
                                        
                                    for guild in GUILDS.to_vec() {
                                        if guild.contains(&id) {
                                            if let Some(code) = guild.split(":").last() {
                                                _ = snipe(code).await;
                                                println!("GUILD UPDATE -> ID : {} - NEW VANITY : {}", id, code);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if payload.contains_key("t") && !payload.get("t").unwrap().is_null() && payload.get("t").unwrap().as_str() == Some("GUILD_AUDIT_LOG_ENTRY_CREATE") {

                        for change in payload.get("d").unwrap().as_object().unwrap().get("changes").unwrap().as_array().unwrap() {

                            if change.as_object().unwrap().contains_key("key") && change.as_object().unwrap().contains_key("old_value") {
                                let key = change.get("key").unwrap().as_str().unwrap();
                                let old_val = change.get("old_value").unwrap().as_str().unwrap();

                                if key.eq("vanity_url_code") {
                                    unsafe {
                                        for guild in GUILDS.to_vec() {
                                            if guild.contains(&old_val) {
                                                _ = snipe(old_val).await;
                                                println!("GUILD LOG -> OLD VANITY : {}", old_val);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                6 => {
                    println!("RESUME!");

                    let selftoken_stored ;
                    let sessionid_stored ;
                    let sequence_stored ;
                    
                    unsafe {
                        selftoken_stored = SELFTOKEN.clone();
                        sessionid_stored = SESSIONID.clone();
                        sequence_stored = SEQ;
                    }

                    let payload = format!("{}{}{}{}{}{}{}", "{\"op\":6,\"d\":{\"token\":\"", selftoken_stored, "\",\"session_id\":\"", sessionid_stored, "\",\"seq\":", sequence_stored, "}}");
                    
                    tx.lock()
                            .await
                            .send(Message::Text(payload.to_string()))
                            .await
                            .unwrap();
                }
                7 => {
                    println!("RECONNECT!");
                    unsafe {
                        SESSIONID = String::from("");
                        SEQ = 0;
                    }
                }
                9 => {
                    println!("INVALID SESSION!");
                    if payload.contains_key("d") && payload.get("d").unwrap().as_bool().unwrap() {
                        let selftoken_stored ;
                        let sessionid_stored ;
                        let sequence_stored ;
                        
                        unsafe {
                            selftoken_stored = SELFTOKEN.clone();
                            sessionid_stored = SESSIONID.clone();
                            sequence_stored = SEQ;
                        }
    
                        let payload = format!("{}{}{}{}{}{}{}", "{\"op\":6,\"d\":{\"token\":\"", selftoken_stored, "\",\"session_id\":\"", sessionid_stored, "\",\"seq\":", sequence_stored, "}}");
                        
                        tx.lock()
                                .await
                                .send(Message::Text(payload.to_string()))
                                .await
                                .unwrap();

                        return;
                    }
                    
                    unsafe {
                        SESSIONID = String::from("");
                        SEQ = 0;
                    }
                }
                10 => {
                    let heartbeat = payload
                        .get("d")
                        .unwrap()
                        .as_object()
                        .unwrap()
                        .get("heartbeat_interval")
                        .unwrap()
                        .as_u64()
                        .unwrap();

                    let tokenstored;

                    unsafe {
                        tokenstored = SELFTOKEN.clone();
                    }

                    let hellopacket = format!("{}{}{}", "{\"op\":2,\"d\":{\"presence\":{\"activities\":[],\"afk\":false,\"since\":0,\"status\":\"online\"},\"capabilities\":0,\"properties\":{\"os\":\"Windows\",\"browser\":\"Discord Client\",\"release_channel\":\"stable\",\"client_version\":\"1.0.9016\",\"os_version\":\"10.0.19045\",\"os_arch\":\"x64\",\"system_locale\":\"en-US\",\"browser_user_agent\":\"Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.9016 Chrome/108.0.5359.215 Electron/22.3.12 Safari/537.36\",\"browser_version\":\"22.3.12\",\"client_build_number\":219839,\"native_build_number\":35780,\"client_event_source\":null},\"compress\":false,\"client_state\":{\"guild_versions\":{},\"highest_last_message_id\":\"0\",\"read_state_version\":0,\"user_guild_settings_version\":-1,\"user_settings_version\":-1,\"private_channels_version\":\"0\",\"api_code_version\":0},\"token\":", String::from(tokenstored), "}}");

                    println!("Hello : {}", hellopacket);

                    tx.lock()
                        .await
                        .send(Message::Text(hellopacket.to_string()))
                        .await
                        .expect("Failed sending authorization payload");
                    
                    loop {
                                                
                        let payload;
                        let seqstored;
                        
                        unsafe {
                            seqstored = SEQ;
                        }

                        if seqstored == 0 {
                            payload = serde_json::json!({
                                "op": 1,
                                "d": serde_json::Value::Null
                            });
                        } else {
                            payload = serde_json::json!({
                                "op": 1,
                                "d": seqstored
                            });
                        }

                        unsafe {
                            SEQ += 1;
                        }

                        println!("Sending heartbeat: {:?}", payload);
                        tx.lock()
                            .await
                            .send(Message::Text(payload.to_string()))
                            .await
                            .unwrap();
                        time::sleep(time::Duration::from_millis(heartbeat)).await;
                    }
                }
                11 => {}
                _ => {
                    println!("Unhandled OP code: {}", op);
                }
            }
        }
        None => println!("Could not find OP code in payload"),
    }
}

async fn read_file_contents(filename: &str) -> io::Result<String> {
    let mut file = File::open(filename).await?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).await?;
    Ok(contents)
}

#[tokio::main]
async fn main()  {
    
    let file_contents: Result<String, io::Error> = read_file_contents("config.json").await;

    match file_contents {
        Ok(contents) => {
            let config: HashMap<String, serde_json::Value> = serde_json::from_str(&contents).unwrap();
            unsafe {
                SELFTOKEN = config.get("self_token").unwrap().to_string();
                SNIPETOKEN = config.get("snipe_token").unwrap().to_string();
                GUILDID = config.get("guild_id").unwrap().to_string();

                for vanity in config.get("vanities").unwrap().as_array().unwrap() {
                    VANITIES.push(vanity.to_string());
                }
            }
        }
        Err(err) => {
            eprintln!("Error reading file: {}", err);
        }
    }

    let config = WebSocketConfig {
        max_frame_size: Some(32 * 1024 * 1024),
        ..Default::default()
    };

    let (socket, _) = connect_async_with_config(GATEWAY_URL, Some(config), false).await.unwrap();
    let (tx, mut rx) = socket.split();

    let tx = Arc::new(Mutex::new(tx));

    while let Some(msg) = rx.next().await {
        match msg {
            Ok(Message::Text(msg)) => {
                tokio::spawn(handle_payload(Arc::clone(&tx), msg));
            }
            Ok(_) => {
                println!("Unknown message");
            }
            Err(err) => {
                eprintln!("Error receiving message: {:?}", err);
            }
        }
    }
}
