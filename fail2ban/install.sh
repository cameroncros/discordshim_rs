#!/bin/bash

sudo cp discordshim.local /etc/fail2ban/jail.d/
sudo cp discordshim.conf /etc/fail2ban/filter.d/

sudo systemctl restart fail2ban

./check.sh
