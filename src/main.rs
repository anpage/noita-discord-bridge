use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use poise::serenity_prelude::ChannelId;
use tokio::io;
use tokio::sync::broadcast::Sender;

mod discord;
mod irc;

pub struct Channel {
    /// Name of streaming "channel" created by the Discord bot for Noita to join
    name: String,
    /// Broadcast Sender used to pass messages from Discord to associated Noita instances
    tx: Sender<Signal>,
}

pub type Channels = Arc<Mutex<HashMap<ChannelId, Channel>>>;

/// Holds persistent state for the Discord bot framework
pub struct State {
    /// Associations between Discord's ChannelId and IRC channel name
    pub channels: Channels,
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
    },
    /// The TCP connection should be severed
    Disconnect,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let channels = Arc::new(Mutex::new(HashMap::new()));
    let channels_discord = channels.clone();

    let (_, result) = tokio::join!(
        irc::run(channels),
        discord::build_framework(channels_discord).run()
    );
    result.unwrap();

    Ok(())
}
