use crate::{Channel, Channels, Signal, State};
use log::{debug, error, info};
use poise::serenity_prelude as serenity;
use rand::seq::SliceRandom;
use tokio::sync::broadcast;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, State, Error>;

pub async fn run_framework(token: String, channels: Channels) -> Result<(), ()> {
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
                            let channels = data.channels.lock();
                            if let Some(c) = channels.get(&new_message.channel_id) {
                                let msg = Signal::UserMessage {
                                    name: new_message.author.name.clone(),
                                    message: new_message.content.clone(),
                                };
                                debug!("Sending message: {:?}", msg);
                                if let Err(e) = c.tx.send(msg) {
                                    if c.tx.receiver_count() > 0 {
                                        error!("Couldn't send message to TCP stream thread: {}", e);
                                    }
                                }
                            }
                        }
                        poise::Event::Ready { .. } => {
                            info!("Discord bot ready.");
                        }
                        _ => {}
                    }
                    Ok(())
                })
            },
            ..Default::default()
        })
        .token(token)
        .intents(
            serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT,
        )
        .user_data_setup(move |_ctx, _ready, _framework| {
            Box::pin(async move { Ok(State { channels }) })
        });

    match framework.run().await {
        Ok(r) => Ok(r),
        Err(e) => {
            error!("Failed to run Discord bot: {e}");
            Err(())
        }
    }
}

/// Command to instruct the bot to assign a "channel name" to this Discord channel,
/// which must be typed into Noita's Twitch channel name to connect.
///
/// While this "channel name" is active, all messages in this Discord channel are
/// passed to the associated Noita instance.
#[poise::command(prefix_command)]
async fn noita(ctx: Context<'_>) -> Result<(), Error> {
    debug!("Received noita command. Picking channel.");

    let channel_id = ctx.channel_id();

    let channel = {
        let mut channels = ctx.data().channels.lock();
        if let Some(c) = channels.get(&channel_id) {
            c.name.clone()
        } else {
            let mut rng = rand::thread_rng();
            let mut channel = (*memorable_wordlist::WORDS
                .choose(&mut rng)
                .expect("The channel word list is somehow empty"))
            .to_string();
            debug!("Trying channel {}", channel);

            while channels.values().any(|c| c.name == channel) {
                debug!("Trying channel {}", channel);
                channel = (*memorable_wordlist::WORDS
                    .choose(&mut rng)
                    .expect("The channel word list is somehow empty"))
                .to_string();
            }
            debug!("Decided on channel {}", channel);
            let (tx, _) = broadcast::channel::<Signal>(32);
            channels.insert(
                channel_id,
                Channel {
                    name: channel.to_string(),
                    tx,
                },
            );
            channel
        }
    };

    ctx.say(format!("Here's your Noita channel name:\n`{}`", channel))
        .await?;

    Ok(())
}

/// Command to instruct the bot to stop listening to messages in a channel.
///
/// This frees up the Noita channel name as well.
#[poise::command(prefix_command)]
async fn noitastop(ctx: Context<'_>) -> Result<(), Error> {
    debug!("Received noitastop command. Deleting channel.");
    let deleted = {
        let mut channels = ctx.data().channels.lock();
        channels.remove(&ctx.channel_id())
    };
    if let Some(c) = deleted {
        ctx.say(format!("Noita streaming channel `{}` deleted.", c.name))
            .await?;
        c.tx.send(Signal::Disconnect)?;
    } else {
        ctx.say("There isn't currently a Noita streaming channel to stop.")
            .await?;
    }
    Ok(())
}
