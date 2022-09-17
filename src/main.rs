use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use poise::serenity_prelude::ChannelId;
use tokio::io;
use tokio::sync::broadcast::{self, Sender};

mod discord;
mod irc;

pub type Channels = Arc<Mutex<HashMap<ChannelId, String>>>;

/// The overall application state
pub struct State {
    /// Associations between Discord's ChannelId and IRC channel name
    pub channels: Channels,
    /// Sender for messages from Discord to Noita
    pub tx: Sender<Signal>,
}

#[derive(Clone, Debug)]
/// Signals passed from Discord to Noita
pub enum Signal {
    /// A message sent by a Discord user
    UserMessage {
        /// Username of message sender
        name: String,
        /// Content of message
        message: String,
        /// Channel to send the message to
        channel: String,
    },
    /// The TCP connection should be severed
    Disconnect { channel: String },
}

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let (tx, _) = broadcast::channel::<Signal>(32);
    let tx_discord = tx.clone();

    let channels = Arc::new(Mutex::new(HashMap::new()));
    let channels_discord = channels.clone();

    let (_, result) = tokio::join!(
        irc::run(tx, channels),
        discord::build_framework(tx_discord, channels_discord).run()
    );
    result.unwrap();

    Ok(())
}
