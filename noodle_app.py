#!/usr/bin/env python

import os
import re
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

global BIG_WORDLIST
global SMALL_WORDLIST


def handle_noodle_input(input_text, cursor):
    nxn_match = re.match(r"^nxn ([0-9]+):(.*)$", input_text)
    if nxn_match:
        n_words_str, nx_expr = nxn_match.groups()
        print(nx_expr, n_words_str)
        nx = Nx.new(nx_expr)
        n_words = int(n_words_str)
        iterate = lambda output: nx.combo_match(
            BIG_WORDLIST, n_words=n_words, cursor=cursor, output=output
        )
        query_text = "    nxn {}: {}".format(n_words, nx_expr)
    else:
        filters = [
            Filter.new_from_spec(s.strip()) for s in input_text.split("\n") if s.strip()
        ]
        iterate = lambda output: filter_chain_apply(
            filters, BIG_WORDLIST, cursor=cursor, output=output
        )
        query_text = "\n".join([f.debug() for f in filters])

    first = True
    output = None
    next_output = 0
    while True:
        output = iterate(output)

        output_text = ""
        output_text += "#0 {}\n".format(cursor.debug())
        output_text += "#1 {} matches\n".format(len(output))

        if first:
            output_text += "\nQuery:\n{}\n\n".format(query_text)
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
    SMALL_WORDLIST = WordList.new_from_file("/usr/share/dict/words", False)
    print("Loaded small wordlist:", SMALL_WORDLIST.debug())

    BIG_WORDLIST = WordList.new_from_file("consolidated.txt", True)
    print("Loaded bigwordlist:", BIG_WORDLIST.debug())

    server = HTTPServer(("localhost", 8080), NoodleHandler)
    print("Running webserver")
    server.serve_forever()
