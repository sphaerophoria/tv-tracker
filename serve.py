#!/usr/bin/env python3

from argparse import ArgumentParser
from pathlib import Path
from http.server import HTTPServer, SimpleHTTPRequestHandler


def parse_arguments():
    parser = ArgumentParser()
    parser.add_argument("--snapshot-dir", required=True)

    return parser.parse_args()


SNAPSHOT_DIR = None


class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=SNAPSHOT_DIR, **kwargs)

    def send_head(self):
        output_file = Path(self.directory) / self.path[1:]
        print(output_file)
        print(output_file.is_dir())
        if output_file.is_dir():
            self.path = self.path + "/self"
        return super().send_head()


def main(snapshot_dir):
    global SNAPSHOT_DIR
    SNAPSHOT_DIR = snapshot_dir
    server_address = ("localhost", 8889)
    httpd = HTTPServer(server_address, Handler)
    httpd.serve_forever()
    pass


if __name__ == "__main__":
    main(**vars(parse_arguments()))
