use grammers_client::types::PackedChat;
use grammers_client::Client;

#[derive(Clone, Debug)]
pub struct ChatItem {
    pub name: String,
    pub packed: PackedChat,
    pub is_archived: bool,
}

pub async fn fetch_chat_list(
    client: &Client,
    limit: usize,
) -> Result<Vec<ChatItem>, Box<dyn std::error::Error>> {
    let mut dialogs = client.iter_dialogs();
    let mut main_chats = Vec::new();
    let mut archived_chats = Vec::new();

    while let Some(dialog) = dialogs.next().await? {
        let chat = dialog.chat();
        let item = ChatItem {
            name: chat.name().to_string(),
            packed: chat.pack(),
            is_archived: dialog.raw.pinned(),
        };

        if item.is_archived {
            archived_chats.push(item);
        } else {
            main_chats.push(item);
        }

        if main_chats.len() + archived_chats.len() >= limit {
            break;
        }
    }

    // Archived chats section always sits at the top
    let mut result = Vec::new();
    result.extend(archived_chats);
    result.extend(main_chats);

    Ok(result)
}
