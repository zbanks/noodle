#!/usr/bin/env python3

# Get the latest Wikipedia dump from here:
# https://dumps.wikimedia.org/other/cirrussearch/current/
#   enwiki-*-cirrussearch-content.json.gz
#   enwiktionary-*-cirrussearch-content.json.gz
#
# Urban dictionary from (extract from archive.zip):
# https://www.kaggle.com/therohk/urban-dictionary-words-dataset
#   urbandict-word-defs.csv

import collections
import datetime
import gzip
import orjson as json
import re
import sys

invalid_re = re.compile(r"([0-9@<>{}]|http|\[|\])")
canonicalize_re = re.compile(r"[^a-zA-Z']")


def dump_words(words, filename):
    with open(filename, "w") as wf:
        for word, count in words.most_common():
            if not word:
                continue
            wf.write("{} {}\n".format(count, word))


def parse_wiki(f, name, update_rate=1e6):
    title_filename = "out/{}-titles.txt".format(name)
    words_filename = "out/{}-words.txt".format(name)
    update_rate = int(update_rate)

    words = collections.Counter()
    n_titles = 0
    with open(title_filename, "w") as titles:
        for line in f:
            obj = json.loads(line)
            title = obj.get("title")
            if title is None:
                continue
            text = obj.get("text")
            if text is None:
                continue
            titles.write(title)
            titles.write("\n")

            ws = [w for w in text.split(" ") if not invalid_re.search(w)]
            words.update([canonicalize_re.sub("", w) for w in ws])

            n_titles += 1
            if n_titles % update_rate == 0:
                print(
                    "{}: {} {} titles {} words".format(
                        str(datetime.datetime.now()), name, n_titles, len(words),
                    )
                )
                dump_words(words, words_filename)
    dump_words(words, words_filename)


def parse_urban_csv(f, name, update_rate=2e5):
    title_filename = "out/{}-titles.txt".format(name)
    words_filename = "out/{}-words.txt".format(name)
    update_rate = int(update_rate)

    words = collections.Counter()
    n_titles = 0
    with open(title_filename, "w") as titles:
        for line in f:
            row = line.split(",", 5)
            title = row[1]
            text = row[5]
            if not title:
                continue

            titles.write(title)
            titles.write("\n")

            ws = [w for w in text.split(" ") if not invalid_re.search(w)]
            words.update([canonicalize_re.sub("", w) for w in ws])

            n_titles += 1
            if n_titles % 10000 == 0:
                print(
                    "{}: {} {} titles {} words".format(
                        str(datetime.datetime.now()), name, n_titles, len(words),
                    )
                )
                dump_words(words, words_filename)
    dump_words(words, words_filename)


def consolidate():
    # (file, weight_b, weight_e, min_count)
    files = [
        # ("out/enwiktionary-titles.txt", 1000, 1, 1),
        ("raw/words", 500, None, None),
        ("out/enwiki-titles.txt", 300, None, None),
        ("out/urbandict-titles.txt", 100, None, None),
        # ("out/enwiktionary-words.txt", 100, 5),
        ("out/enwiki-words.txt", 20, 0.2, 5),
        ("out/urbandict-words.txt", 10, 0.1, 5),
    ]
    words = {}
    for filename, weight_b, weight_e, min_count in files:
        print("Loading {}".format(filename))
        with open(filename) as f:
            for line in f:
                w = line.strip()
                if min_count is not None:
                    count, w = w.split(" ", 1)
                    count = int(count)
                    if count < min_count:
                        continue
                    weight = weight_b + count ** weight_e
                else:
                    weight = weight_b
                canonical = canonicalize_re.sub("", w).lower().replace("'", "")
                if not canonical:
                    continue
                if canonical in words:
                    if min_count is None:
                        words[canonical][0] = max(words[canonical][0], weight)
                    else:
                        words[canonical][0] += weight
                else:
                    words[canonical] = [weight, w]

    with open("out/consolidated.txt", "w") as f:
        for weight, w in sorted(words.values(), key=lambda x: x[0], reverse=True):
            f.write("{} {}\n".format(int(weight), w))


def main():
    t = sys.argv[1]
    if t == "consolidate":
        consolidate()
    elif t == "enwiki":
        # This takes about 60 minutes
        with gzip.open(
            "raw/enwiki-20201019-cirrussearch-content.json.gz", mode="r"
        ) as f:
            parse_wiki(f, "enwiki")
    elif t == "enwiktionary":
        # This takes about 5 minutes
        with gzip.open(
            "raw/enwiktionary-20201019-cirrussearch-content.json.gz", mode="r"
        ) as f:
            parse_wiki(f, "enwiktionary")
    elif t == "urbandict":
        # This takes about 3 minutes
        with open("urbandict-word-defs.csv") as f:
            parse_urban_csv(f, "urbandict")
    else:
        print("Unknown type '{}'".format(t))


if __name__ == "__main__":
    main()
