#!/usr/bin/env python3
# A static server for the local preview that never lets a browser keep a stale page: pages and manifests are sent with no-cache,
# bundles (their names carry a content hash) and the pinned js-dos files are immutable.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/preview-serve.py <dir> [port] [--bind ADDRESS]      (port 8080 and 127.0.0.1 unless given)
import argparse
import functools
import http.server
import re
import socketserver


class Handler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        path = self.path.split("?", 1)[0]
        if re.search(r"\.[0-9a-f]{10}\.jsdos$", path) or "/js-dos/" in path:
            self.send_header("Cache-Control", "public, max-age=31536000, immutable")
        else:
            self.send_header("Cache-Control", "no-cache, must-revalidate")
        super().end_headers()


class Server(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True
    allow_reuse_address = True


def main():
    ap = argparse.ArgumentParser(description="Serve the local preview with headers that keep a browser from holding a stale page.")
    ap.add_argument("dir")
    ap.add_argument("port", nargs="?", type=int, default=8080)
    ap.add_argument("--bind", default="127.0.0.1", help="address to listen on (0.0.0.0 shows the preview to the network)")
    a = ap.parse_args()
    print("serving %s on http://%s:%d/" % (a.dir, a.bind, a.port), flush=True)
    Server((a.bind, a.port), functools.partial(Handler, directory=a.dir)).serve_forever()


if __name__ == "__main__":
    main()
