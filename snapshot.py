#!/usr/bin/env python3

import json
import shutil

from argparse import ArgumentParser
from http.client import HTTPConnection
from http import HTTPStatus
from pathlib import Path
from urllib.parse import urlparse


def parse_arguments():
    parser = ArgumentParser()
    parser.add_argument("--server-url", required=True)
    parser.add_argument("--snapshot-dir", required=True)

    return parser.parse_args()


class SnapshotWriter:
    def __init__(self, connection, snapshot_dir):
        self.snapshot_dir = Path(snapshot_dir)
        self.connection = connection

    def snap(self, url):
        self.connection.request("GET", url)
        response = self.connection.getresponse()

        if response.status != HTTPStatus.OK:
            raise RuntimeError("Unexpected failure in get shows")

        body = response.read()

        path = self.snapshot_dir / url[1:] / "self"
        path.parent.mkdir(exist_ok=True, parents=True)
        with open(path, "wb") as f:
            f.write(body)

        return body


def write_shows_to_snapshot(snapshot_writer):
    shows = json.loads(snapshot_writer.snap("/shows"))
    for show in shows:
        snapshot_writer.snap("/shows/" + show)
        snapshot_writer.snap("/shows/" + show + "/episodes")

    return shows


def write_ratings_to_snapshot(snapshot_writer):
    ratings = json.loads(snapshot_writer.snap("/ratings"))
    for rating in ratings:
        snapshot_writer.snap("/ratings/" + rating)


def write_show_images_to_snapshot(shows, snapshot_writer):
    for show_id in shows:
        show = shows[show_id]
        image_id = show.get("image", None)
        if image_id is not None:
            snapshot_writer.snap("/images/" + str(image_id))


def copy_resource_dir_to_output(snapshot_dir):
    resource_dir = Path(__file__).parent / "res/client"
    shutil.copytree(resource_dir, snapshot_dir)


def main(snapshot_dir, server_url):
    copy_resource_dir_to_output(snapshot_dir)

    server_url = urlparse(server_url)
    connection = HTTPConnection(server_url.hostname, server_url.port)
    snapshot_writer = SnapshotWriter(connection, snapshot_dir)

    shows = write_shows_to_snapshot(snapshot_writer)
    write_ratings_to_snapshot(snapshot_writer)
    write_show_images_to_snapshot(shows, snapshot_writer)


if __name__ == "__main__":
    main(**vars(parse_arguments()))
