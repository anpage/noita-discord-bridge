#![warn(clippy::all)]

use std::collections::HashMap;
use std::sync::Arc;

use env_logger::Env;
use log::error;
use parking_lot::Mutex;
use poise::serenity_prelude::ChannelId;
use tokio::io;
use tokio::sync::broadcast::Sender;
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, LinesCodec};

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
    env_logger::Builder::from_env(Env::default().default_filter_or("noita_discord_bridge=info"))
        .format_target(false)
        .init();

    let channels = Arc::new(Mutex::new(HashMap::new()));
    let channels_discord = channels.clone();

    let token = {
        if let Ok(token) = std::env::var("DISCORD_TOKEN") {
            token
        } else {
            error!("DISCORD_TOKEN is missing from your environment. Please provide one now:");
            let stdin = io::stdin();
            let mut reader = FramedRead::new(stdin, LinesCodec::new());
            reader.next().await.unwrap().unwrap()
        }
    };

    tokio::try_join!(
        irc::run(channels),
        discord::run_framework(token, channels_discord)
    )
    .unwrap();

    Ok(())
}
