#!/usr/bin/env python

import os
import re
from itertools import zip_longest
from http.server import BaseHTTPRequestHandler, HTTPServer

from noodle import (
    Word,
    WordSet,
    WordList,
    WordSetAndBuffer,
    Nx,
    Cursor,
    nx_combo_multi,
    now_ns,
    error_get_log,
)

CHUNK_TIME_NS = 50e6  # 50ms
TOTAL_TIME_NS = 15e9  # 15s

WORDLIST_SOURCES = [
    # "consolidated.txt",
    # "/usr/share/dict/american-english-insane",
    "/usr/share/dict/words",
]


def gen_anagram(anagram):
    """rearrange the given letters"""
    letters = set(anagram)
    # Length constraint
    yield "[%s]{%d}" % ("".join(sorted(letters)), len(anagram))

    # Must contain the right number of each letter
    for l in letters:
        s = "[%s]*" % "".join(sorted(letters - {l}))
        yield s.join([""] + [l] * anagram.count(l) + [""])


def gen_subanagram(anagram):
    """rearrange at most the given letters"""
    letters = set(anagram)

    # Length constraint
    yield "[%s]{1,%d}" % ("".join(sorted(letters)), len(anagram))

    # Must contain at most # of each letter
    for l in letters:
        s = "[%s]*" % "".join(sorted(letters - {l}))
        yield s.join([""] + ["%s?" % l] * anagram.count(l) + [""])


def gen_superanagram(anagram):
    """rearrange at least the given letters"""
    letters = set(anagram)

    # Length constraint
    yield ".{%d,}" % len(anagram)

    # Must contain the at least the right number of each letter
    for l in letters:
        yield ".*".join([""] + [l] * anagram.count(l) + [""])


def gen_transdelete(anagram, n=1):
    """rearrange all but n of the given letters"""
    if n >= len(anagram):
        raise Exception("can't transdelete {} letters from {}".format(n, anagram))
    letters = set(anagram)

    # Length constraint
    yield "[%s]{%d}" % ("".join(sorted(letters)), len(anagram) - n)

    # Must contain at most # of each letter
    for l in letters:
        s = "[%s]*" % "".join(sorted(letters - {l}))
        yield s.join([""] + ["%s?" % l] * anagram.count(l) + [""])


def gen_transadd(anagram, n=1):
    """rearrange all of the given letters plus n wildcards"""
    letters = set(anagram)

    # Length constraint
    yield ".{%d}" % (len(anagram) + n)

    # Must contain the at least the right number of each letter
    for l in letters:
        yield ".*".join([""] + [l] * anagram.count(l) + [""])


def expand_expression(expression):
    expression = expression.lower().strip()
    if not expression:
        return []

    if re.match(r"[0-9 ]+", expression):
        # Enumeration
        # TODO: handle ' or - in enumerations (e.g. "1 3'1 5")
        counts = re.split(r" +", expression)
        return [Nx.new("_" + "_".join("." * int(c) for c in counts) + "_")]

    expression = expression.replace(" ", "")

    # Substring "(...:?)"
    expression = re.sub(
        r"\(([a-z_-]+):\?\)", lambda m: "({}?)".format("?".join(m.group(1))), expression
    )

    if "<" in expression:
        parts = re.split(r"<(.+?)(:?)([+~-]?)(\d?)>", expression)
        plains, anagrams, colons, plusminuses, ns = (
            parts[0::5],
            parts[1::5],
            parts[2::5],
            parts[3::5],
            parts[4::5],
        )
        assert len(plains) == len(anagrams) + 1

        terms = []
        for anagram, colon, plusminus, n in zip(anagrams, colons, plusminuses, ns):
            anagram = anagram.lower()
            assert anagram != ""

            if (colon, plusminus, n) == ("", "", ""):
                terms.append(list(gen_anagram(anagram)))
            elif colon == "":
                print("a", anagram, "c", colon, "pm", plusminus, "n", n)
                if plusminus not in ("+", "-"):
                    raise Exception(
                        "Invalid `<...>` group: `<{}>`".format(
                            anagram + colon + plusminus + n
                        )
                    )
                if n == "":
                    if plusminus == "+":
                        terms.append(list(gen_superanagram(anagram)))
                    elif plusminus == "-":
                        terms.append(list(gen_subanagram(anagram)))
                else:
                    if plusminus == "+":
                        terms.append(list(gen_transadd(anagram, int(n))))
                    elif plusminus == "-":
                        terms.append(list(gen_transdelete(anagram, int(n))))
            else:
                raise Exception(
                    "Invalid `<...>` group: `<{}>`".format(
                        anagram + colon + plusminus + n
                    )
                )

        nxs = []
        for ts in zip_longest(*terms, fillvalue=".*"):
            assert len(ts) + 1 == len(plains)
            expression = plains[0]
            for t, p in zip(ts, plains[1:]):
                expression += t + p
            nxs.append(Nx.new(expression))
        return nxs

    if ":" not in expression:
        return [Nx.new(expression)]


def handle_noodle_input(input_text, output, cursor):
    nxs = []
    for line in input_text.split("\n"):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        nxs.extend(expand_expression(line))

    if not nxs:
        yield "#0 No input"
        return

    iterate = lambda: nx_combo_multi(
        nxs, WORDLIST, n_words=10, cursor=cursor, output=output,
    )
    query_text = "".join(["    {}\n".format(f.debug()) for f in nxs])

    first = True
    next_output = 0
    while True:
        iterate()

        output_text = ""
        output_text += "#0 {}\n".format(cursor.debug())
        output_text += "#1 {} matches\n".format(len(output))

        if first:
            output_text += "\nExpanded Query:\n{}\n".format(query_text)
            first = False

        for i in range(next_output, len(output)):
            word = output[i]
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

        error_get_log()
        try:
            output = WordSetAndBuffer()
            cursor = Cursor.new_to_wordset(
                output.wordlist,
                output,
                deadline_ns=now_ns() + CHUNK_TIME_NS,
                deadline_output_index=300,
            )
            total_deadline_ns = now_ns() + TOTAL_TIME_NS

            for chunk in handle_noodle_input(data, output, cursor):
                try:
                    self.wfile.write(chunk.encode("utf-8"))
                except BrokenPipeError:
                    print("Connection closed")
                    break
                if cursor.is_done() or now_ns() > total_deadline_ns:
                    break
                cursor.set_deadline(now_ns() + CHUNK_TIME_NS)
        except Exception as e:
            self.wfile.write(
                "Encountered exception while processing query:\n    {}\n\n".format(
                    e
                ).encode("utf-8")
            )
            self.wfile.write(
                "Internal logs:\n\n{}".format(error_get_log()).encode("utf-8")
            )
            raise e


def load_wordlist():
    global WORDLIST
    for filename in WORDLIST_SOURCES:
        if os.path.exists(filename):
            WORDLIST = WordList.new_from_file(filename)
            print("Loaded wordlist:", WORDLIST.debug())
            return
    raise Exception(
        "No wordlist found from {} candidates".format(len(WORDLIST_SOURCES))
    )


if __name__ == "__main__":
    load_wordlist()

    server = HTTPServer(("0", 8080), NoodleHandler)
    print("Running webserver")
    server.serve_forever()
