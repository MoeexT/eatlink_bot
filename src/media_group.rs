use std::{collections::HashMap, time::Duration};

use lazy_static::lazy_static;
use teloxide::{types::Message, Bot};
use tokio::{sync::Mutex, time::sleep};


#[derive(Debug, Clone)]
enum MediaType {
    Photo(String), // file-id
    Video(String), // file-id
}

lazy_static! {
    static ref MEDIA_GROUP_CACHE: Mutex<HashMap<String, Vec<MediaType>>> = Mutex::new(HashMap::new());
}


async fn handle_media_group(bot: Bot, msg: Message) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(photo) = msg.photo().and_then(|photos| photos.last()) {
        let file_id = photo.file.id.clone();
        process_media(bot.clone(), msg.clone(), MediaType::Photo(file_id)).await?;
    }
    if let Some(video) = msg.video() {
        let file_id = video.file.id.clone();
        process_media(bot.clone(), msg.clone(), MediaType::Video(file_id)).await?;
    }
    Ok(())
}

async fn process_media(bot: Bot, msg: Message, media: MediaType) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(media_group_id) = msg.media_group_id() {
        let mut cache = MEDIA_GROUP_CACHE.lock().await;
        let group = cache.entry(media_group_id.clone()).or_insert_with(Vec::new);
        group.push(media.clone());

        if group.len() == 2
    }
    
    Ok(())
}

async fn clean_cache() {
    loop {
        sleep(Duration::from_secs(30)).await;
        let mut cache = MEDIA_GROUP_CACHE.lock().await;
        cache.retain(|_, photos| photos.len() < 2);
    }
}

