#!/usr/bin/env python

import os
import re
import unicodedata
from itertools import zip_longest
from http.server import BaseHTTPRequestHandler, HTTPServer
import urllib

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
TOTAL_TIME_NS = 120e9  # 120s
N_WORDS_DEFAULT = 10
OUTPUT_LIMIT_DEFAULT = 300

WORDLIST_SOURCES = {
    "small": "/usr/share/dict/american-english-small",
    "default": "/usr/share/dict/words",
    "large": "/usr/share/dict/american-english-large",
    "huge": "/usr/share/dict/american-english-huge",
    # These are slow to load
    # "insane": "/usr/share/dict/american-english-insane",
    # "all": "consolidated.txt",
}

WORDLISTS = {}
DEFAULT_WORDLIST = None


def load_wordlist(wordlist_filename, preprocess=False):
    raw_words = []
    with open(wordlist_filename) as f:
        for line in f:
            word = line.strip()

            # Normalize all non-[a-z] characters
            word = (
                word.replace("æ", "ae")
                .replace("Æ", "AE")
                .replace("œ", "oe")
                .replace("Œ", "OE")
            )
            word = unicodedata.normalize("NFKD", word)

            if preprocess:
                # Remove all one-letter words except "a" & "I"
                if len(word) < 2 and word not in ("a", "I"):
                    continue

            raw_words.append(word)

    if preprocess:
        # Sort words by length & if they contain special characters
        values_and_words = []
        for word in raw_words:
            value = 0
            value += len(word) ** 2
            if re.match(r"^[a-z]+$", word):
                value += 100
            elif word.lower() == word:
                value += 10
            values_and_words.append((value, word))
        raw_words = list(zip(*sorted(values_and_words, reverse=True)))[1]

    wordlist = WordList.new()
    for word in raw_words:
        wordlist.add(word)
    return wordlist


def load_wordlists():
    for name, filename in WORDLIST_SOURCES.items():
        if os.path.exists(filename):
            wl = load_wordlist(
                filename, preprocess=filename.startswith("/usr/share/dict/")
            )
            WORDLISTS[name] = wl
            print("Loaded wordlist {} from {}: {}".format(name, filename, wl.debug()))
    if not WORDLISTS:
        raise Exception(
            "No wordlist found from {} candidates".format(len(WORDLIST_SOURCES))
        )


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


def expand_expression(expression, replacements):
    print(">", expression)

    # Collapse whitespace
    expression = re.sub(r"\s+", " ", expression)

    # Remove comments
    expression = re.sub(r"#.*", "", expression)

    # Macro definition
    if "=" in expression:
        name, _eq, value = expression.partition("=")
        name = name.strip()
        value = value.strip()
        assert "=" not in value, "Expression contained multiple '=' characters"
        assert name not in replacements, "Macro '{}' redefined".format(name)
        replacements[name] = value
        return []

    # Macro replacement
    for name, value in replacements.items():
        # NB: This requires Python 3.6+ dict ordering
        expression = expression.replace(name, value)

    # Convert to lowercase
    expression = expression.lower()

    # Flags: "!_", "!'", and "!2"
    flags = 0
    if "!_" in expression:
        flags |= Nx.Flags.EXPLICIT_SPACE
        expression = expression.replace("!_", "", 1)
    if "!'" in expression:
        flags |= Nx.Flags.EXPLICIT_PUNCT
        expression = expression.replace("!'", "", 1)
    fuzzy_flag = re.search(r"![0-9]", expression)
    if fuzzy_flag:
        fuzzy_flag = fuzzy_flag.group(0)
        flags |= int(fuzzy_flag[1:])
        expression = expression.replace(fuzzy_flag, "", 1)

    # Enumerations (e.g. "1 3'1 5")
    if re.match(r"^[0-9' ]+$", expression):
        full_expr = ""
        for term in re.split(r"([0-9]+)", expression):
            if term in ("", " "):
                full_expr += "_"
            elif re.match(r"^[0-9]+$", term):
                full_expr += "." * int(term)
            else:
                full_expr += term
        flags |= Nx.Flags.EXPLICIT_SPACE
        return [Nx.new(full_expr, flags=flags)]

    # Remove whitespace entirely
    expression = expression.replace(" ", "")

    # Substring "(...:?)"
    expression = re.sub(
        r"\(([a-z_-]+):\?\)", lambda m: "({}?)".format("?".join(m.group(1))), expression
    )

    # <...> terms
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
            nxs.append(Nx.new(expression, flags=flags))
        return nxs

    if not expression:
        return []

    return [Nx.new(expression, flags=flags)]


def handle_noodle_input(input_text, output, cursor):
    wordlist_name = "default"
    n_words = N_WORDS_DEFAULT
    replacements = {}
    quiet = False
    nxs = []
    for line in input_text.replace(";", "\n").split("\n"):
        line = line.strip()
        if line.startswith("#quiet"):
            quiet = True
        elif line.startswith("#list"):
            wordlist_name = line.split()[1]
        elif line.startswith("#words"):
            n_words = int(line.split()[1])
        elif line.startswith("#limit"):
            limit = int(line.split()[1])
            cursor.set_deadline(deadline_output_index=limit)
        else:
            nxs.extend(expand_expression(line, replacements))

    if not nxs:
        yield "#0 No input\n" if not quiet else "\n"
        return

    wordlist = WORDLISTS.get(wordlist_name.lower())
    if wordlist is None:
        yield (
            "#0 Unknown wordlist '{}'\n".format(wordlist_name)
            + "#1 Options: {}\n".format(" ".join(WORDLISTS.keys()))
        ) if not quiet else "\n"
        return

    iterate = lambda: nx_combo_multi(
        nxs, wordlist, n_words=n_words, cursor=cursor, output=output,
    )
    query_text = "".join(["    {}\n".format(f.debug()) for f in nxs])

    first = True
    next_output = 0
    while True:
        iterate()

        output_text = ""
        if not quiet:
            output_text += "#0 {}\n".format(cursor.debug())
            output_text += "#1 {} matches from wordlist {} ({})\n".format(
                len(output), wordlist_name, len(wordlist)
            )

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
        if self.path.startswith("/noodle"):
            query = self.path.partition("?")[2]
            query = urllib.parse.unquote(query)
            self.send_response(200)
            self.end_headers()
            return self.handle_query("#quiet\n#limit 15\n" + query)

        path = path.replace("//", "/")
        if self.path.count("/") > 1 or not os.path.exists(path):
            self.send_error(404, "Not Found: {}".format(path))

        with open(path, "rb") as f:
            self.send_response(200)
            self.end_headers()
            self.wfile.write(f.read())

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        query = self.rfile.read(length).decode("utf-8")
        self.send_response(200)
        self.end_headers()
        return self.handle_query(query)

    def handle_query(self, query):
        error_get_log()
        try:
            output = WordSetAndBuffer()
            cursor = Cursor.new_to_wordset(
                output.wordlist,
                output,
                deadline_ns=now_ns() + CHUNK_TIME_NS,
                deadline_output_index=OUTPUT_LIMIT_DEFAULT,
            )
            total_deadline_ns = now_ns() + TOTAL_TIME_NS

            for chunk in handle_noodle_input(query, output, cursor):
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


if __name__ == "__main__":
    load_wordlists()

    bind = ("127.0.0.1", 8081)
    server = HTTPServer(bind, NoodleHandler)
    print("Running webserver on {}".format(bind))
    server.serve_forever()
