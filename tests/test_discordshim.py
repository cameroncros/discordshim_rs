import socket

from _pytest.outcomes import fail

from .livediscordtestcase import LiveDiscordTestCase
from .messages_pb2 import Response, EmbedContent, ProtoFile, TextField

ONE_MEGABYTE = 1 * 1024 * 1024
DISCORD_MAX_FILE_SIZE = 5 * ONE_MEGABYTE
COLOR_INFO = 0x123456

MAX_TITLE = 256
MAX_DESCRIPTION = 4096
MAX_FIELDS = 25
MAX_VALUE = 1024
MAX_FOOTER = 2048
MAX_AUTHOR = 256
MAX_EMBED_TOTAL = 6000


class TestDiscordShim(LiveDiscordTestCase):

    def test_send_without_settings(self):

        try:
            client = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            client.settimeout(10)
            client.connect((self.discordshim_addr, int(self.discordshim_port)))

            response = Response(embed=EmbedContent())
            data = response.SerializeToString()
            client.sendall(len(data).to_bytes(4, byteorder='little'))
            client.sendall(data)
        except:
            fail(f"Failed to connect to {self.discordshim_addr}:{self.discordshim_port}")

    def test_send_minimal_embed(self):
        self.start_scraper()

        response = Response(embed=EmbedContent())
        data = response.SerializeToString()
        self.client.sendall(len(data).to_bytes(4, byteorder='little'))
        self.client.sendall(data)

        self.stop_scraper(waitformessages=1)

    def test_send_embed_mention(self):
        self.start_scraper()

        response = Response(embed=EmbedContent(title="", description="<@481274558238949415>"))
        data = response.SerializeToString()
        self.client.sendall(len(data).to_bytes(4, byteorder='little'))
        self.client.sendall(data)

        self.stop_scraper(waitformessages=1)

    def test_send_complete_embed(self):
        self.start_scraper()

        response = Response(
            embed=EmbedContent(
                author="Author",
                title="Title",
                description="description",
                color=COLOR_INFO,
                snapshot=ProtoFile(data=self.snapshot_bytes, filename="snapshot.png"),
                textfield=[TextField(title="Title"), TextField(text="Text")]
            )
        )
        data = response.SerializeToString()
        self.client.sendall(len(data).to_bytes(4, byteorder='little'))
        self.client.sendall(data)

        results = self.stop_scraper(waitformessages=1)
        self.assertIn("snapshot.png", results[0].embeds[0].image.url)

    def test_send_small_file(self):
        self.start_scraper()

        file = ProtoFile(data=b"Hello World", filename="Helloworld.txt")
        response = Response(file=file)
        data = response.SerializeToString()
        self.client.sendall(len(data).to_bytes(4, byteorder='little'))
        self.client.sendall(data)

        results = self.stop_scraper(waitformessages=1)
        self.assertIn("Helloworld.txt", results[0].attachments[0].filename)

    def test_send_large_file(self):
        self.start_scraper()

        with open('/dev/urandom', 'rb') as f:
            data = f.read(6 * ONE_MEGABYTE)

        file = ProtoFile(data=data, filename="Helloworld.dat")
        response = Response(file=file)
        data = response.SerializeToString()
        self.client.sendall(len(data).to_bytes(4, byteorder='little'))
        self.client.sendall(data)

        results = self.stop_scraper(waitformessages=7)
        self.assertIn("Helloworld.dat", results[0].content)
        self.assertIn("Helloworld.dat.zip.001", results[1].attachments[0].filename)
        self.assertIn("Helloworld.dat.zip.002", results[2].attachments[0].filename)
        self.assertIn("Helloworld.dat.zip.003", results[3].attachments[0].filename)
        self.assertIn("Helloworld.dat.zip.004", results[4].attachments[0].filename)
        self.assertIn("Helloworld.dat.zip.005", results[5].attachments[0].filename)
        self.assertIn("Helloworld.dat.zip.006", results[6].attachments[0].filename)

    def test_embedbuilder(self):
        self.start_scraper()
        textfields = [TextField(title='c' * MAX_TITLE, text='d' * MAX_VALUE, inline=True) for _ in
                      range(MAX_FIELDS + 1)]

        response = Response(
            embed=EmbedContent(
                title='a' * MAX_TITLE,
                description='b' * MAX_DESCRIPTION,
                author='c' * MAX_AUTHOR,
                color=COLOR_INFO,
                textfield=textfields
            )
        )
        data = response.SerializeToString()
        self.client.sendall(len(data).to_bytes(4, byteorder='little'))
        self.client.sendall(data)

        results = self.stop_scraper(waitformessages=8)
        self.assertEqual('a' * MAX_TITLE, results[0].embeds[0].title)
        self.assertEqual('b' * MAX_DESCRIPTION, results[0].embeds[0].description)
        self.assertEqual('c' * MAX_AUTHOR, results[0].embeds[0].author.name)

    def test_unicode_embed(self):
        self.start_scraper()

        teststr = "٩(-̮̮̃-̃)۶ ٩(●̮̮̃•̃)۶ ٩(͡๏̯͡๏)۶ ٩(-̮̮̃•̃)."
        response = Response(
            embed=EmbedContent(
                title=teststr,
                description=teststr,
                author=teststr,
                textfield=[TextField(title=teststr, text=teststr)]
            )
        )
        data = response.SerializeToString()
        self.client.sendall(len(data).to_bytes(4, byteorder='little'))
        self.client.sendall(data)

        results = self.stop_scraper(waitformessages=1)
        self.assertEqual(teststr, results[0].embeds[0].title)
        self.assertEqual(teststr, results[0].embeds[0].description)
        self.assertEqual(teststr, results[0].embeds[0].author.name)
        self.assertEqual(teststr, results[0].embeds[0].fields[0].name)
        self.assertEqual(teststr, results[0].embeds[0].fields[0].value)
