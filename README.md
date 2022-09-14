# Noita Discord Bridge
A Discord bot that acts as a minimal Twitch chat server and allows you to use Noita's Twitch integration inside a Discord chat channel.

## Getting Started
There's currently no publicly hosted instance of this bot, so you'll need to host it yourself or on a cloud provider.

### Docker
There's a Dockerfile provided, which you can use to quickly get the bot up and running:

1. `docker build -t noita-discord-bridge .`
2. `docker run -it -e DISCORD_TOKEN=[bot token here] -p 6667:6667 --rm --name noita-discord-bridge`

### Noita
Once your bot is invited to your server and the Docker container is running, you must set a line in your `hosts` file appropriately to intercept Noita's connection to Twitch chat:

`127.0.0.1 irc.chat.twitch.tv`

(Change `127.0.0.1` to the IP address of the machine running the bot)

### Discord
Type `!noita` in the Discord chat channel where you want people to be able to interact with your game. The bot will give you a channel name to type into Noita's Twitch "Channel name" field in Options.

Click "Connect" and you should be good to go!