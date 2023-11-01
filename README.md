# discordshim_rs

## Purpose

The purpose of this project is to act as a middleware between
the [OctoPrint-DiscordRemote](https://github.com/cameroncros/OctoPrint-DiscordRemote) plugin and discord.
Originally, the plugin used discord.py, however, as discord.py advanced, it started to drop support for older python version.
Separating the Discord and Octoprint parts of the plugin was the solution chosen, however, it has a few pros and cons:

### Pros

- Complete separation and decoupling of discord and plugin.
- Simpler API, but only supports a subset of discord functionality.
- Multi-tenant, a single DiscordShim can support multiple Octoprint plugins
- No more requirement on creating a discord bot, new users can just use the existing bot
- Power users can self-host DiscordShim for themselves.

### Cons

- There are privacy concerns, as the central DiscordBot sees all messages on all servers that it is added to.
- 

## Installation

### Self Built

Fastest way to setup the bot is to use the Docker Compose scripts.

```shell
cat <<EOT >> .env
BOT_TOKEN='$LIVE_BOT_TOKEN'
EXTERNAL_PORT=23416
EOT
docker-compose up --build -d
```

### Pre-built

If that doesn't work, your platform may not be supported for building the docker container.
Instead, use the prebuilt image with:

```shell
cat <<EOT >> .env
BOT_TOKEN='$LIVE_BOT_TOKEN'
EXTERNAL_PORT=23416
EOT
docker-compose -f docker-compose-prebuilt.yml up -d
```

### Raspian/OctoPi

If you want to deploy to OctoPi, first you will need to install docker:

```shell
sudo apt update && sudo apt install docker.io
sudo systemctl enable --now docker
```

And then deploy with:
```shell
cat <<EOT >> .env
BOT_TOKEN='$LIVE_BOT_TOKEN'
EXTERNAL_PORT=23416
EOT
docker-compose -f docker-compose-prebuilt-armv7.yml up -d
```

## Development

### CI

CI is hosted on a private gitlab, this is a mirror.
If you wish to make a contribution, make a fork and MR as per standard github processes,
and the merge will be made by the myself manually.

### Testing

#### Rust Unit Tests

There are some rust unit tests, but no where near covered well enough.

Run with `cargo test`

#### Python System Tests

The python tests allow testing the shim end-to-end.

Run with:
```shell
# Start DiscordShim
BOT_TOKEN=$LIVE_BOT_TOKEN cargo run

# Start tests
BOT_TOKEN=$LIVE_BOT_TOKEN DISCORDSHIM_ADDR=127.0.0.1 DISCORDSHIM_PORT=53416 pytest
```