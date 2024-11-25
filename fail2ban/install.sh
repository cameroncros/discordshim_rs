#!/bin/bash

sudo cp discordshim.local /etc/fail2ban
sudo cp discordshim.conf /etc/fail2ban/filters.d/

sudo systemctl restart fail2ban
sudo fail2ban-client status discordshim