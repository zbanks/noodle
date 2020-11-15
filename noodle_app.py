#!/usr/bin/env python

from noodle import Word, WordSet, WordList, Filter, Nx, filter_chain_apply

from http.server import BaseHTTPRequestHandler, HTTPServer
import os

global WORDLIST


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
        print("Post:", data)
        self.send_response(200)
        self.end_headers()

        filters = [
            Filter.new_from_spec(s.strip()) for s in data.split("\n") if s.strip()
        ]
        output = filter_chain_apply(filters, WORDLIST)
        output.sort_value()

        self.wfile.write(
            "{} match(es) for {} filter(s):\n".format(len(output), len(filters)).encode(
                "utf-8"
            )
        )
        for f in filters:
            self.wfile.write("    {}\n".format(f.debug()).encode("utf-8"))
        self.wfile.write(b"\n\n")

        if len(output):
            max_value = output[0].value
            format_str = "{:<" + str(len(str(max_value)) + 4) + "}{}\n"
            for word in output:
                self.wfile.write(
                    format_str.format(word.value, str(word)).encode("utf-8")
                )


def test():
    w = Word.new("Hello, world!")
    print("word:", str(w), repr(w))

    wl = WordList.new_from_file("/usr/share/dict/words")
    wl.add("Hello, world!", 2000)
    wl.wordset.sort_value()
    print(wl.debug())

    f = Filter.new("regex", arg_str="hell?o.*")
    print(f)
    print(f.apply(wl.wordset).debug())

    spec = """
    extract: ab(.{7})
    extractq: .(.*).
    nx 1: .*in
    """.strip()
    filters = [Filter.new_from_spec(s.strip()) for s in spec.split("\n")]
    print(filter_chain_apply(filters, wl).debug())

    n = Nx.new("helloworld")
    print(n.combo_match(wl.wordset, 3).debug())


if __name__ == "__main__":
    test()

    WORDLIST = WordList.new_from_file("consolidated.txt", True)
    print("Loaded wordlist:", WORDLIST.debug())

    server = HTTPServer(("localhost", 8080), NoodleHandler)
    print("Running webserver")
    server.serve_forever()
