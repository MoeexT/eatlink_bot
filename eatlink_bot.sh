#!/bin/bash

docker run -d \
    --network=host \
    -e RUST_LOG=DEBUG \
    -e TELOXIDE_TOKEN='7616932833:AAG9EdzWf3IxvoqgfxWVfhHx227sy7vCcIU' \
    -v /mnt/sdb1/AriaDownload/Telegram:/downloads \
    --name eatlink-bot \
    eatlink-bot:0.1.0
