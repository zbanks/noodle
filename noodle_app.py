#!/usr/bin/env python

import os
from http.server import BaseHTTPRequestHandler, HTTPServer

from noodle import (
    Word,
    WordSet,
    WordList,
    Filter,
    Nx,
    Cursor,
    filter_chain_apply,
    now_ns,
)

global WORDLIST


def handle_noodle_input(input_text, cursor):
    filters = [
        Filter.new_from_spec(s.strip()) for s in input_text.split("\n") if s.strip()
    ]

    first = True
    output = None
    next_output = 0
    while True:
        output = filter_chain_apply(filters, WORDLIST, cursor=cursor, output=output)

        output_text = ""
        output_text += "#0 {}\n".format(cursor.debug())
        output_text += "#1 {} match(es) for {} filter(s):\n".format(
            len(output), len(filters)
        )

        if first:
            output_text += "\nInput Query:\n"
            for f in filters:
                output_text += "    {}\n".format(f.debug())
            output_text += "\n"
            first = False

        for i in range(next_output, len(output)):
            word = output[i]
            # output_text += "{:<10}{}\n".format(word.value, str(word))
            output_text += "{}\n".format(str(word))
        next_output = len(output)

        yield output_text


class NoodleHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        path = "static/" + self.path
        if self.path == "/":
            path = "static/index.html"
        path = path.replace("//", "/")
        if self.path.count("/") > 1 or not os.path.exists(path):
            self.send_error(404, "Not Found: {}".format(path))

        with open(path) as f:
            self.send_response(200)
            self.end_headers()
            self.wfile.write(f.read().encode("utf-8"))

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        data = self.rfile.read(length).decode("utf-8")
        self.send_response(200)
        self.end_headers()

        CHUNK_TIME_NS = 50e6  # 50ms
        TOTAL_TIME_NS = 15e9  # 15s

        cursor = Cursor.new()
        cursor.set_deadline(now_ns() + CHUNK_TIME_NS, deadline_output_index=300)
        total_deadline_ns = now_ns() + TOTAL_TIME_NS

        for chunk in handle_noodle_input(data, cursor):
            try:
                self.wfile.write(chunk.encode("utf-8"))
            except BrokenPipeError:
                print("Connection closed")
                break
            if cursor.is_done() or now_ns() > total_deadline_ns:
                break
            cursor.set_deadline(now_ns() + CHUNK_TIME_NS)


if __name__ == "__main__":
    WORDLIST = WordList.new_from_file("consolidated.txt", True)
    # WORDLIST = WordList.new_from_file("/usr/share/dict/words", False)
    print("Loaded wordlist:", WORDLIST.debug())

    server = HTTPServer(("localhost", 8080), NoodleHandler)
    print("Running webserver")
    server.serve_forever()
