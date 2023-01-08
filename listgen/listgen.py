#!/usr/bin/env python
from collections import Counter
from dataclasses import dataclass, field
from multiprocessing import Pool
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple
import math
import orjson as json
import re
import sys
import unicodedata
import zstandard as zst


@dataclass
class Wordlist:
    word_data: Dict[str, Tuple[int, int, str]] = field(default_factory=dict)

    @staticmethod
    def load(path: Path) -> "Wordlist":
        wordlist = Wordlist()
        with path.open() as f:
            for i, line in enumerate(f):
                row = line.strip().split("\t")
                score, word = row[0], row[-1]
                wordlist.add_word(word=word, index=i, score=int(score))
        return wordlist

    def dump(self, path: Path) -> None:
        with path.open("w") as f:
            for canonical, (score, _, word) in sorted(
                self.word_data.items(), key=lambda x: x[1][0], reverse=True
            ):
                f.write(f"{score}\t{canonical}\t{word}\n")
                # f.write(f"{score}\t{word}\n")
        print(f"Saved {len(self.word_data)}-entry wordlist to {path}")

    def dump_final(self, path: Path) -> None:
        total_scores = sum(x[0] for x in self.word_data.values())
        max_score = max(x[0] for x in self.word_data.values())
        min_score = min(x[0] for x in self.word_data.values())
        min_frequency = min_score / total_scores
        scale = 1e6 / math.log(min_frequency)
        assert scale < 0
        with path.open("w") as f:
            for canonical, (score, _, word) in sorted(
                self.word_data.items(), key=lambda x: x[1][0], reverse=True
            ):
                value = int(scale * math.log(score / total_scores))
                f.write(f"{value}\t{word}\n")
        print(f"Saved {len(self.word_data)}-entry wordlist to {path}")

    def add_canonical(self, canonical: str, word: str, score: int) -> int:
        index = len(self.word_data)
        self.word_data[canonical] = (score, index, word)
        return index

    def add_word(
        self, word: str, index: Optional[int] = None, score: int = 0
    ) -> Optional[str]:
        if index is None:
            index = len(self.word_data)
        word = strip_word(word)
        if not word:
            return None
        canonical = canoncialize(word)
        if canonical not in self.word_data:
            self.word_data[canonical] = (score, index, word)
        else:
            old_score, old_index, old_word = self.word_data[canonical]
            self.word_data[canonical] = (old_score + score, old_index, old_word)
        return canonical

    def merge(self, other: "Wordlist") -> None:
        for canonical, (score, _, word) in other.word_data.items():
            if canonical not in self.word_data:
                self.word_data[canonical] = (score, len(self.word_data), word)
            else:
                old_score, old_index, old_word = self.word_data[canonical]
                self.word_data[canonical] = (old_score + score, old_index, old_word)

    def canonical_score(self, canonical: str) -> int:
        data = self.word_data.get(canonical)
        if not data:
            return 0
        return data[0]


strip_body_re = re.compile(r"(^.*? Retrieved .*?) Retrieved ")
strip_word_re = re.compile(r"^\W*([\w/\-,.&']*\w)\W*$")
any_letter_re = re.compile(r".*[a-zA-Z].*")
# canonicalize_re = re.compile(r"^.*?([a-zA-Z'/\-,.&']*).*?$")
domain_re = re.compile(r".*\.(com|co\.|org|net|gov|biz|info|today|cz|at|dk|de)")


def strip_body(body: str) -> str:
    # Many bodies end in a series of References, which we'd like to ignore.
    # It's hard to perfectly remove them from the cirrussearch JSON, but they are usually
    # indicated with something like "Retrieved January 1st, 2022".
    # Cut off the article starting at the second " Retrieved ", which should hopefully
    # exclude all but the first reference.
    match = strip_body_re.match(body)
    if match:
        return match.group(1)
    return body


def strip_word(word: str) -> str:
    # Even before canonicalizing, some strings don't count as words at all.
    # - The word must have at least one latin letter
    # - Remove any non-alphanumeric characters from the ends of the word
    # - The word cannot contain certain punctuation (like brackets or ":")
    # - Replace "–" with "-"
    # - The word cannot contain "www." or ".com" or other common TLDs
    #     - (Usually domains are filterd out by the ":" check, this is a backstop of sorts)
    if not any_letter_re.match(word):
        return ""
    match = strip_word_re.match(word)
    if not match:
        return ""
    word = match.group(1)
    if "_" in word:
        return ""
    word = word.replace("–", "-")
    if "www." in word or domain_re.match(word):
        return ""
    return word


def canoncialize(word: str) -> str:
    if any("0" <= x <= "9" for x in word):
        return ""
    canonical = word
    canonical = (
        canonical.replace("æ", "ae")
        .replace("Æ", "AE")
        .replace("œ", "oe")
        .replace("Œ", "OE")
    )
    canonical = "".join(
        c
        for c in unicodedata.normalize("NFKD", canonical)
        if not unicodedata.combining(c)
    )
    try:
        canonical = canonical.casefold().encode("ascii").decode("ascii")
    except UnicodeEncodeError:
        return ""
    while canonical and not "a" <= canonical[0] <= "z":
        canonical = canonical[1:]
    while canonical and not "a" <= canonical[-1] <= "z":
        canonical = canonical[:-1]
    # canonical = canonicalize_re.match(canonical).group(1)
    return canonical


def alpha_ratio(word: str) -> float:
    alphas = sum("a" <= l <= "z" for l in word.casefold())
    return alphas / len(word)


def extract_word_frequency(json_file: Path) -> None:
    title_counter: Counter[str] = Counter()
    body_counter: Counter[str] = Counter()
    with zst.open(json_file, mode="r", newline="\n") as f:
        for i, line in enumerate(f):
            if i % 100000 == 0:
                print(f"> {i} {json_file} {len(title_counter)} {len(body_counter)}")
            title, score, body = json.loads(line)
            if title is None or body is None:
                continue
            title_counter.update(strip_word(w) for w in title.split())
            body_counter.update(strip_word(w) for w in strip_body(body).split())
    print(f"> done {json_file} {len(title_counter)} {len(body_counter)}")

    with json_file.with_suffix(".title-wordlist").open("w") as f:
        for word, count in title_counter.items():
            f.write(f"{count}\t{word}\n")

    with json_file.with_suffix(".body-wordlist").open("w") as f:
        for word, count in body_counter.items():
            f.write(f"{count}\t{word}\n")


def split_word_frequency(base_path: Path) -> None:
    # Parallelize
    pool = Pool(8)
    files = list(base_path.glob("enwiki-*.json.*.zst"))
    #files = [f for f in files if any(x in str(f) for x in ("00", "05", "06", "07"))]
    print(files)
    pool.map(extract_word_frequency, files)


def word_frequency(base_path: Path) -> None:

    # Merge
    title_count: Dict[str, int] = {}
    body_count: Dict[str, int] = {}
    for p in base_path.glob("enwiki-*.title-wordlist"):
        with p.open() as f:
            for line in f:
                #count_str, _, word = line.strip().split("\t")
                count_str, word = line.strip("\n").split("\t")
                title_count[word] = title_count.get(word, 0) + int(count_str)
        print(f"loaded {p}")
    for p in base_path.glob("enwiki-*.body-wordlist"):
        with p.open() as f:
            for line in f:
                #count_str, _, word = line.strip().split("\t")
                count_str, word = line.strip("\n").split("\t")
                body_count[word] = body_count.get(word, 0) + int(count_str)
        print(f"loaded {p}")

    canonical_form: Dict[str, str] = {}
    for c in (title_count, body_count):
        new_c: Counter[str] = Counter()
        for word, count in c.items():
            canonical = canoncialize(word)
            if canonical not in canonical_form:
                c_count = title_count.get(canonical, 0) + body_count.get(canonical, 0)
                if c_count >= 0.01 * count:
                    # The canonicalized form also exists (at least 1% of the uses), so prefer that
                    canonical_form[canonical] = canonical
                else:
                    # The non-canonicalized form is predominant, so it's a proper noun, abbreviation, etc.
                    canonical_form[canonical] = word
            new_c[canonical_form[canonical]] += count
        c.clear()
        c.update(new_c)
    print(f"merged counts")

    with (base_path / "title.txt").open("w") as f:
        for word, score in title_count.items():
            f.write(f"{score}\t{word}\n")
    print("dumped title")

    with (base_path / "body.txt").open("w") as f:
        for word, score in body_count.items():
            f.write(f"{score}\t{word}\n")
    print("dumped body")


def create_wordlist(base_path: Path, cutoff: int = 10) -> Wordlist:
    word_points: Dict[str, int] = {}
    original_words: Dict[str, str] = {}
    blocklist: Set[str] = {"aeo"}  # From the album name "æo³ & ³hæ"

    def add_word(count: int, word: str) -> None:
        if not word:
            return
        assert "\t" not in word
        canonical = canoncialize(word)
        if not canonical or canonical in blocklist:
            return
        if canonical == word:
            # If the word matches its canonical form, give it a big bonus
            count = count * 2 + 1
        elif canonical == word.casefold():
            # If the word matches its canonical form except for capitalization, give it a small bonus
            count = int(count * 1.5) + 1
        elif len(canonical) / len(word) <= 0.5:
            # If the "word" shrinks by >50% when canonicalizing, give it a major penalty
            count = int(count * 0.2 - 1)
        elif alpha_ratio(word) <= 0.75:
            # If the "word" is less than 75% letters, give it a minor penalty
            count = int(count * 0.5 - 1)

        points = word_points.get(canonical, 0)
        points += count
        word_points[canonical] = points
        original_words[canonical] = word

    with (base_path / "enwiktionary.txt").open() as f:
        for line in f:
            count_str, _, word = line.strip().split("\t")
            # Words in wiktionary are already scaled to 100/1000 points
            add_word(int(count_str), word)

    with (base_path / "title.txt").open() as f:
        for line in f:
            count_str, _, word = line.strip().partition("\t")
            # Words in the title get a 3x bonus as they're less likely to be typos
            add_word(int(count_str) * 3, word)

    with (base_path / "body.txt").open() as f:
        for line in f:
            count_str, _, word = line.strip().partition("\t")
            add_word(int(count_str), word)

    #with open("/home/zbanks/wordlist_consolidated.txt") as f:
    #    for line in f:
    #        add_word(25, line.strip())

    output_words = [(c, w) for w, c in word_points.items() if c >= cutoff]
    output_words.sort(reverse=True)
    print(f"Cutoff = {cutoff}; picking {len(output_words)} / {len(word_points)} words")

    wordlist = Wordlist()
    for count, word in output_words:
        wordlist.add_canonical(canonical=word, word=original_words[word], score=count)

    wordlist_path = base_path / f"wordlist.{cutoff}.txt"
    print(f"Saving wordlist to {wordlist_path}")
    wordlist.dump_final(wordlist_path)
    return wordlist


def validate_wordlist(wordlist: Wordlist) -> None:
    answer_scores: Counter[int] = Counter()
    for answer in load_mh_answers(Path("/home/zbanks/mh_answers/")):
        if len(answer) <= 4 and ord(answer[0]) > 0x1F000:
            continue
        score, _, _ = wordlist.word_data.get(canoncialize(answer), (0, 0, ""))
        log_score = int(math.log2(score)) if score > 0 else 0
        answer_scores[log_score] += 1
        if score == 0:
            print(f"Missing answer: {answer}")
    for i in range(30):
        print(f"> score ~ {2**i}, count={answer_scores.get(i, 0)}")

    total_words: int = 0
    missing_words: int = 0
    base_wordlist_path = Path("/home/zbanks/wordlist_consolidated.txt")
    with open("missing_words.txt", "w") as o:
        with base_wordlist_path.open() as f:
            for line in f:
                if "'s" in line:
                    continue
                if canoncialize(line.strip()) not in wordlist.word_data:
                    o.write(line)
                    missing_words += 1
                total_words += 1
    print(f"> Missing {missing_words} / {total_words} from {base_wordlist_path}")


def wiktionary_wordlist(path: Path) -> Wordlist:
    wordlist = Wordlist()
    with zst.open(path, mode="r", newline="\n") as f:
        for line in f:
            data = json.loads(line.strip())
            if data.get("lang_code") != "en" or "word" not in data:
                continue
            wordlist.add_word(data["word"], score=1000)
            for form in data.get("forms", []):
                form_word = form["form"]
                if " " not in form_word and canoncialize(form_word) == form_word:
                    wordlist.add_word(form["form"], score=50)
    return wordlist


def load_mh_answers(mh_answers_path: Path) -> List[str]:
    answers = []
    for year in mh_answers_path.glob("mystery*.txt"):
        with year.open() as f:
            for line in f:
                answer, _, kind = line.strip().partition(",")
                answers.extend(answer.replace("-", " ").replace("'S", "").split())
    return answers


def build_wiki_graph(input_file, wordlist: Wordlist, base_path: Path) -> None:
    graph: Dict[int, Set[int]] = {}

    def words_to_set(line: str) -> Set[int]:
        output: Set[int] = set()
        for word in line.split():
            word = canoncialize(strip_word(word))
            data = wordlist.word_data.get(word)
            if data is not None and data[1] > 1000:
                output.add(data[1])
        return output

    try:
        for line in input_file:
            if len(graph) > 100000:
                break
            title, score, body = json.loads(line)
            if title is None or body is None:
                continue
            title_words = words_to_set(title)
            body_words = words_to_set(strip_body(body))
            for t in title_words:
                if t not in graph:
                    graph[t] = set()
                graph[t].update(body_words)
    except:
        pass

    print()
    print()
    print(len(graph))
    print(sum(len(x) for x in graph.values()))
    print()
    print()


def main() -> None:
    base_path = Path("enwiki-index")
    base_path.mkdir(parents=True, exist_ok=True)
    dict_wordlist = wiktionary_wordlist(base_path / "kaikki-enwiktionary.json.zst")
    dict_wordlist.dump(base_path / "enwiktionary.txt")
    #split_word_frequency(base_path)
    word_frequency(base_path)
    wordlist = create_wordlist(base_path, cutoff=50)
    # wordlist = Wordlist.load(base_path / "wordlist.10.txt")
    #validate_wordlist(wordlist)
    # build_wiki_graph(sys.stdin, wordlist, base_path)


if __name__ == "__main__":
    main()
