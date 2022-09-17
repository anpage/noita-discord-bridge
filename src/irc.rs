use futures::SinkExt;
use irc_proto::{Command, Message};
use log::{debug, error, info};
use tokio::{
    io::{self, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::broadcast::{Receiver, Sender},
};
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LinesCodec};

use crate::{Channels, Signal};

pub async fn run(tx: Sender<Signal>, channels: Channels) -> io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:6667").await?;
    info!("Listening for connections...");
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
}

/// Handler for incoming TCP/IRC connections from Noita. It implements the bare
/// minimum to fool Noita into thinking that it's connected to Twitch chat.
pub async fn process_socket(socket: TcpStream, mut rx: Receiver<Signal>, channels: Channels) {
    let mut irc_stream = Framed::new(socket, LinesCodec::new());
    let mut username = "bar".to_string();
    let mut channel = "foo".to_string();

    loop {
        tokio::select! {
        Ok(signal) = rx.recv() => {
            debug!("Message received");
            match signal {
                Signal::UserMessage { name, message, channel: chan } => {
                    if chan == channel {
                        debug!("(MESSAGE) #{}: {}: {}", chan, name, message);
                        let _ = irc_stream.send(format!("@badge-info=;@display-name={}; PRIVMSG #{} :{}\r\n", name, chan, message)).await;
                    }
                }
                Signal::Disconnect { channel: chan } => {
                    if chan == channel {
                        debug!("Killing connection for channel {channel}.");
                        irc_stream.into_inner().shutdown().await.unwrap();
                        break;
                    }
                }
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
                                if channels.lock().unwrap().values().any(|c| format!("#{c}") == chan) {
                                    channel = chan.trim_start_matches("#").to_string();
                                    if let Err(e) = irc_stream.send(format!(":{username}!{username}@{username}.tmi.twitch.tv JOIN #{channel}\r\n:{username}.tmi.twitch.tv 353 {username} = #{channel} :{username}\r\n:{username}.tmi.twitch.tv 366 {username} #{channel} :End of /NAMES list\r\n")).await {
                                        error!("error on sending response; error = {:?}", e);
                                    }
                                } else {
                                    if let Err(e) = irc_stream.send(format!(":.tmi.twitch.tv NOTICE #{channel} :Channel doesn't exist\r\n")).await {
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
