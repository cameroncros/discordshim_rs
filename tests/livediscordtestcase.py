import asyncio
import os
import socket
import threading
import time
from typing import List
from unittest import TestCase

import discord
from _pytest.outcomes import fail
from discord.message import Message

from . import _get_path
from .messages_pb2 import Response, Settings


class Scraper:
    lock = threading.Lock()
    running = True
    messages = []

    def __init__(self, bot_token: str, channel_id: str):
        self.is_connected = False
        self.messages = []
        self.bot_token = bot_token
        self.channel_id = channel_id
        self.client = discord.Client(intents=discord.Intents.all())

        @self.client.event
        async def on_message(message):
            if message.channel.id != int(channel_id) and message.channel.type.name != "private":
                # Only care about messages from correct channel, or DM messages
                return

            self.lock.acquire()
            self.messages.append(message)
            self.lock.release()

        @self.client.event
        async def on_ready():
            self.lock.acquire()
            self.is_connected = True
            self.lock.release()
            asyncio.create_task(self.wait_for_shutdown())

    async def wait_for_shutdown(self):
        while self.running:
            await asyncio.sleep(1)
        await self.client.close()

    def run(self):
        self.client.run(self.bot_token)

    def stop(self):
        self.running = False


class LiveDiscordTestCase(TestCase):
    channel_id: str
    bot_token: str
    discordshim_addr: str
    discordshim_port: str
    client: socket
    thread: threading.Thread
    snapshot_bytes: bytes = []

    scraper_pid: int

    def start_scraper(self):
        self.scraper = Scraper(self.bot_token, self.channel_id)
        self.scraper_thread = threading.Thread(target=self.scraper.run)
        self.scraper_thread.start()

        while not self.scraper.is_connected:
            time.sleep(1)

    def stop_scraper(self, waitformessages, timeout=300) -> List[Message]:
        count = 0
        while len(self.scraper.messages) < waitformessages:
            time.sleep(1)
            count += 1
            if count > timeout:
                raise TimeoutError()
        self.scraper.stop()
        self.scraper_thread.join()
        return self.scraper.messages

    @classmethod
    def setUp(self):
        self.bot_token = os.environ['BOT_TOKEN']
        self.channel_id = os.environ['CHANNEL_ID']
        self.discordshim_addr = os.environ.get('DISCORDSHIM_ADDR', 'opdrshim.uk')
        self.discordshim_port = os.environ.get('DISCORDSHIM_PORT', '23416')

        try:
            self.client = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            self.client.settimeout(10)
            self.client.connect((self.discordshim_addr, int(self.discordshim_port)))
            response = Response(
                settings=Settings(channel_id=int(self.channel_id))
            )
            data = response.SerializeToString()
            self.client.sendall(len(data).to_bytes(4, byteorder='little'))
            self.client.sendall(data)
        except:
            fail(f"Failed to connect to {self.discordshim_addr}:{self.discordshim_port}")

    @classmethod
    def setUpClass(cls) -> None:
        with open(_get_path("test_data/test_pattern.png"), "rb") as f:
            cls.snapshot_bytes = f.read()
