use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures::SinkExt;
use irc_proto::{Command, Message};
use log::{debug, error, info};
use poise::serenity_prelude::{self as serenity, ChannelId};
use rand::seq::SliceRandom;
use tokio::io;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::{self, Receiver, Sender};
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LinesCodec};

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;
struct Data {
    channels: Arc<Mutex<HashMap<ChannelId, String>>>,
    tx: Sender<UserMessage>,
}

#[derive(Clone, Debug)]
struct UserMessage {
    name: String,
    message: String,
    channel: String,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:6667").await?;
    env_logger::init();

    let (tx, _) = broadcast::channel::<UserMessage>(32);
    let tx_discord = tx.clone();

    info!("Listening for connections...");

    let channels = Arc::new(Mutex::new(HashMap::new()));
    let channels_discord = channels.clone();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![noita(), noitastop()],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("!".to_string()),
                ..Default::default()
            },
            listener: |_ctx, event, _framework, data| {
                Box::pin(async move {
                    debug!("Got an event in listener: {:?}", event.name());
                    match event {
                        poise::Event::Message { new_message } => {
                            let channels = data.channels.lock().unwrap();
                            if let Some(c) = channels.get(&new_message.channel_id) {
                                let msg = UserMessage {
                                    name: new_message.author.name.clone(),
                                    message: new_message.content.clone(),
                                    channel: c.clone(),
                                };
                                debug!("Sending message: {:?}", msg);
                                if let Err(e) = data.tx.send(msg) {
                                    error!("Couldn't send message to TCP stream thread: {}", e);
                                }
                            }
                        }
                        _ => {}
                    }
                    Ok(())
                })
            },
            ..Default::default()
        })
        .token(std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN"))
        .intents(
            serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT,
        )
        .user_data_setup(move |_ctx, _ready, _framework| {
            Box::pin(async move {
                Ok(Data {
                    channels: channels_discord,
                    tx: tx_discord,
                })
            })
        });

    let (_, result) = tokio::join!(
        async move {
            loop {
                match listener.accept().await {
                    Ok((socket, addr)) => {
                        info!("New connection from {}", addr);
                        tokio::spawn(process_socket(socket, tx.subscribe(), channels.clone()));
                    }
                    Err(e) => {
                        error!("Couldn't accept TCP connection: \n{}", e);
                    }
                }
            }
        },
        framework.run()
    );
    result.unwrap();

    Ok(())
}

/// Command to instruct the bot to stop listening to messages in a channel.
///
/// This frees up the Noita channel name as well.
#[poise::command(prefix_command)]
async fn noita(ctx: Context<'_>) -> Result<(), Error> {
    debug!("Received noita command. Picking channel.");

    let channel_id = ctx.channel_id();

    let mut channel: String;
    {
        let mut channels = ctx.data().channels.lock().unwrap();
        if let Some(c) = channels.get(&channel_id) {
            channel = c.clone();
        } else {
            let mut rng = rand::thread_rng();
            channel = memorable_wordlist::WORDS
                .choose(&mut rng)
                .unwrap()
                .to_string();
            debug!("Trying channel {}", channel);

            while channels.values().any(|c| c == &channel) {
                debug!("Trying channel {}", channel);
                channel = memorable_wordlist::WORDS
                    .choose(&mut rng)
                    .unwrap()
                    .to_string();
            }
            debug!("Decided on channel {}", channel);
            channels.insert(channel_id, format!("#{}", channel.to_string()));
        }
    }

    ctx.say(format!("Here's your Noita channel name:\n`{}`", channel))
        .await?;

    Ok(())
}

/// Command to instruct the bot to assign a "channel name" to this Discord channel,
/// which must be typed into Noita's Twitch channel name to connect.
///
/// While this "channel name" is active, all messages in this Discord channel are
/// passed to the associated Noita instance.
#[poise::command(prefix_command)]
async fn noitastop(ctx: Context<'_>) -> Result<(), Error> {
    debug!("Received noitastop command. Deleting channel.");
    let deleted = {
        let mut channels = ctx.data().channels.lock().unwrap();
        channels.remove(&ctx.channel_id())
    };
    if let Some(c) = deleted {
        ctx.say(format!("Noita streaming channel `{}` deleted.", c))
            .await?;
    } else {
        ctx.say("There isn't currently a Noita streaming channel to stop.")
            .await?;
    }
    Ok(())
}

/// Handler for incoming TCP/IRC connections from Noita. It implements the bare
/// minimum to fool Noita into thinking that it's connected to Twitch chat.
async fn process_socket(
    socket: TcpStream,
    mut rx: Receiver<UserMessage>,
    channels: Arc<Mutex<HashMap<ChannelId, String>>>,
) {
    let mut irc_stream = Framed::new(socket, LinesCodec::new());
    let mut username = "bar".to_string();
    let mut channel = "#foo".to_string();

    loop {
        tokio::select! {
        Ok(msg) = rx.recv() => {
            debug!("Message received");
            if msg.channel == channel {
                debug!("Message is for this channel.");
                let _ = irc_stream.send(format!("@display-name={}; PRIVMSG {} :{}\r\n", msg.name, msg.channel, msg.message)).await;
            }
        }
        result = irc_stream.next() => match result {
                Some(Ok(line)) => {
                    let message: Result<Message, _> = line.parse();
                    if let Ok(msg) = message {
                        debug!("Message: {}", msg);
                        match msg.command {
                            Command::NICK(nick) => {
                                username = nick;
                                if let Err(e) = irc_stream.send(format!(":tmi.twitch.tv 001 {username} :Welcome, GLHF!\r\n:tmi.twitch.tv 002 {username} :Your host is tmi.twitch.tv\r\n:tmi.twitch.tv 003 {username} :This server is rather new\r\n:tmi.twitch.tv 004 {username} :-\r\n:tmi.twitch.tv 375 {username} :-\r\n:tmi.twitch.tv 372 {username} :You are in a maze of twisty passages, all alike.\r\n:tmi.twitch.tv 376 {username} :>\r\n")).await {
                                    error!("error on sending response; error = {:?}", e);
                                }
                            },
                            Command::JOIN(chan, ..) => {
                                if channels.lock().unwrap().values().any(|c| *c == chan) {
                                    channel = chan;
                                    if let Err(e) = irc_stream.send(format!(":{username}!{username}@{username}.tmi.twitch.tv JOIN {channel}\r\n:{username}.tmi.twitch.tv 353 {username} = {channel} :{username}\r\n:{username}.tmi.twitch.tv 366 {username} {channel} :End of /NAMES list\r\n")).await {
                                        error!("error on sending response; error = {:?}", e);
                                    }
                                } else {
                                    if let Err(e) = irc_stream.send(format!(":.tmi.twitch.tv NOTICE {channel} :Channel doesn't exist\r\n")).await {
                                        error!("error on sending response; error = {:?}", e);
                                    }
                                }
                            },
                            _ => {
                                debug!("Command not recognized");
                            },
                        }
                    }

                }
                Some(Err(e)) => {
                    error!("error on decoding from socket; error = {:?}", e);
                }
                None => break,
            }
        }
    }
}
