use std::{
    env::{self},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result};
use chrono::Local;
use once_cell::sync::Lazy;
use teloxide::{
    Bot, RequestError,
    dispatching::dialogue::GetChatId,
    net::Download,
    prelude::*,
    sugar::request::RequestReplyExt,
    types::{Message, MessageId},
};
use tokio::{
    fs::File,
    io::{AsyncWriteExt, BufWriter},
    sync::{
        Mutex,
        mpsc::{Receiver, channel},
    },
    time::sleep,
};

const RECEIVE_TIMEOUT: u64 = 2;
pub static DOWNLOAD_DIR: Lazy<PathBuf> = Lazy::new(|| {
    env::var("DOWNLOAD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let default = Path::new("downloads").to_path_buf();
            default
        })
});

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    ensure_dir_exists(&DOWNLOAD_DIR)?;
    log::info!("Starting bot...");

    let (tx, rx) = channel(20);
    let bot = Arc::new(Bot::from_env());
    let tx = Arc::new(tx);

    let consumer = tokio::spawn(consumer_loop(bot.clone(), rx));
    teloxide::repl(bot.clone(), {
        let tx = tx.clone();
        move |_: Arc<Bot>, msg: Message| {
            let tx = tx.clone();
            async move {
                log::debug!("Send message");
                tx.send(msg).await.unwrap();
                Ok(())
            }
        }
    })
    .await;
    consumer.await??;
    Ok(())
}

struct ConsumerState {
    reply_chat_id: Option<ChatId>,
    reply_message_id: Option<MessageId>,
    statics: Vec<String>,
}

/// Consumer loop:
/// 1. Consumes messages from receiver
/// 2. After messages drain, waits for seconds(default 2 seconds), reply the statistics to the sender.
///
async fn consumer_loop(bot: Arc<Bot>, mut receiver: Receiver<Message>) -> ResponseResult<()> {
    log::info!("Start consumer loop");
    let state = Arc::new(Mutex::new(ConsumerState {
        reply_chat_id: None,
        reply_message_id: None,
        statics: vec![],
    }));
    loop {
        tokio::select! {
            Some(msg) = receiver.recv() => {
                log::info!("Received message: {}", &msg.id);
                let bot = bot.clone();
                let state = state.clone();
                tokio::spawn(async move {
                    log::debug!("Spawn to handle message");
                    let mut state = state.lock().await;
                    state.reply_message_id = state.reply_message_id.or(Some(msg.id.clone()));
                    state.reply_chat_id = state.reply_chat_id.or(msg.chat_id().clone());
                    if let Ok(response) = download(bot, msg).await {
                        state.statics.push(response);
                    }
            });
            },
            _ = sleep(Duration::from_secs(RECEIVE_TIMEOUT)) => {
                let mut state = state.lock().await;
                match (state.reply_chat_id, state.reply_message_id) {
                    (Some(chat_id), Some(msg_id)) => {
                        let response = state.statics.join("\n");
                        bot.send_message(chat_id, &response)
                            .reply_to(msg_id)
                            .await?;
                        state.statics.clear();
                        state.reply_chat_id = None;
                        state.reply_message_id = None;
                        log::info!("Replied statistics message: {}", response);
                    }
                    _ => {}
                }
                continue;
            }
        }
    }
}

async fn download(bot: Arc<Bot>, msg: Message) -> Result<String> {
    log::info!("Handling message: {}", &msg.id);
    let path = DOWNLOAD_DIR.join(Local::now().format("%Y-%m-%d").to_string());
    ensure_dir_exists(&path)?;

    if let Some(photo) = msg.photo().and_then(|p| p.last()) {
        let file_id = &photo.file.id;
        let file = bot.get_file(file_id).await?;
        let file_name = format!("photo_{}.jpg", file_id);
        let path = path.join(&file_name);
        let dst_file = File::create(&path)
            .await
            .map_err(|e| RequestError::Io(Arc::new(e)))?;
        let mut dst_file = BufWriter::new(dst_file);
        log::debug!("Downloading photo: {}", &file_id);
        bot.download_file(&file.path, &mut dst_file).await?;
        log::info!("Downloaded photo: {:?}", &path);
        save_message(&msg, &file_name).await;
        return Ok(format!("下载图片{}成功", &photo.file.unique_id));
    }

    if let Some(video) = msg.video() {
        let file_id = &video.file.id;
        let file = bot.get_file(file_id).await?;
        let file_name = &video
            .file_name
            .clone()
            .or(Some(format!("video_{}.mp4", file_id)))
            .unwrap();
        let path = path.join(&file_name);
        let mut dst_file = File::create(&path)
            .await
            .map_err(|e| RequestError::Io(Arc::new(e)))?;
        log::debug!("Downloading video: {}", &file_id);
        bot.download_file(&file.path, &mut dst_file).await?;
        log::info!("Downloaded video: {:?}", path);
        save_message(&msg, &file_name).await;
        return Ok(format!("下载视频{}成功", file_name));
    }

    Ok(String::from("No media download"))
}

async fn save_message(msg: &Message, file_name: &String) {
    let path = DOWNLOAD_DIR
        .join(Local::now().format("%Y-%m-%d").to_string())
        .join(file_name.to_owned() + ".json");
    let msg_json = serde_json::to_string_pretty(&msg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
    let msg_file = File::create(&path).await;

    match (&msg_file, &msg_json) {
        (Ok(_), Ok(msg_json)) => {
            let mut msg_file = msg_file.unwrap();
            if let Err(e) = msg_file.write_all(msg_json.as_bytes()).await {
                log::warn!("Save json {:?} error: {:?}", &path, e);
            } else {
                log::debug!("Save json {:?} successfully.", &path);
            }
        }
        _ => {
            log::warn!(
                "Save message error: message: {:?}, file:{:?}",
                &msg_file,
                &msg_json
            );
        }
    }
}

fn ensure_dir_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)
            .with_context(|| format!("Create download dir error: {}", path.display()))?;
    }
    Ok(())
}
