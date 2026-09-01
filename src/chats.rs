use grammers_client::types::PackedChat;
use grammers_client::Client;

#[derive(Clone, Debug)]
pub struct MessageItem {
    pub sender: String,
    pub text: String,
    pub date: String,
}

pub async fn fetch_latest_messages(
    client: &Client,
    packed: &PackedChat,
    limit: usize,
    offset_count: usize,
) -> Result<Vec<MessageItem>, Box<dyn std::error::Error>> {
    let mut messages = client.iter_messages(*packed);
    let mut list = Vec::new();
    let mut skipped = 0;

    while let Some(msg) = messages.next().await? {
        if skipped < offset_count {
            skipped += 1;
            continue;
        }

        if !msg.text().is_empty() {
            let sender = msg
                .sender()
                .map(|s| s.name().to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            let date = msg.date().format("%H:%M").to_string();

            list.push(MessageItem {
                sender,
                text: msg.text().to_string(),
                date,
            });
        }

        if list.len() >= limit {
            break;
        }
    }

    list.reverse(); // Prepend older messages at top, newer at bottom
    Ok(list)
}

pub async fn send_text_message(
    client: &Client,
    packed: &PackedChat,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    client.send_message(*packed, text).await?;
    Ok(())
}
