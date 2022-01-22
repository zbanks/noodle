Wordlist Generation
===================

**TODO** - this document & the `listgen.py` tool need significant cleanup.

- Use `enwiki-*-cirrussearch-content.json.gz` files to get a dump of Wikipedia articles with markup stripped.
    - Con: about twice as big as the standard XML dump
    - Con: less frequently mirrored (no torrent available)
    - Con: not compatible with [Nutrimatic tools](https://github.com/ebenolson/nutrimatic)
    - This command will do a streaming download of the JSON, extract the 3 fields we care about, and split the output evenly into 8 files.
        - Con: this can take ~10 hours to download, and isn't resumable.
        - Output is about 9GB total (zstd'd) in Jan 2022. (Original `.json.gz` file is ~30GB).

```
mkdir -p enwiki-index
curl -k https://dumps.wikimedia.org/other/cirrussearch/current/enwiki-20220110-cirrussearch-content.json.gz
    | zcat
    | jq -c '[.title, .popularity_score, .text]'
    | rg -v '^.null,null,null'
    | split --filter='zstd > $FILE.zst' -d -n 'r/8' - 'enwiki-index/enwiki-20220110-cirrussearch-main.json.'
```

- For Wiktionary data, I use the [pre-processed JSON from kaikki.org](https://kaikki.org/dictionary/English/kaikki.org-dictionary-English.json) (1.3GB in early 2022
    - This has already been parsed to isolate the English parts of pages and separated words by "sense"

- `listgen.py`'s current "UI" is editing `main()`
    - Running `split_word_frequency(...)` is currently the slowest step. It extracts word frequency in titles/bodies for each chunk of the `enwiki` dump.
    - The score algorithm & `cutoff` value can be tweaked to set the size of the wordlist

- `listgen.py` validates the output wordlist against past [Mystery Hunt answers](https://github.com/dgulotta/mh_answers)
    - Some answers are straight-up not real words (i.e. `NEWTRITIOUS`, emoji), so it's OK to miss some
    - Some answers would be better written with spaces (`EARTHSCENTER` --> `EARTHS CENTER`).
    - We want the bulk of the answers to be well past (>2x) the cutoff


<!---
- Get latest `enwiki-*-pages-articles.xml.bz2` file, (20GB in early 2022)
    - Fastest download seems to be [via torrent](https://meta.wikimedia.org/wiki/Data_dump_torrents#English_Wikipedia), the non-multistream verison. (Multistream is fine though.)
    - I originally used `enwiki-*-cirrussearch-content.json.gz` files:
        - Pro: already stripped of markup
        - Con: about twice as big as the standard XML dump
        - Con: less frequently mirrored
    - All these steps can be repeated for `enwiktionary-*-pages-articles.xml.bz2` for wiktionary
- Use `remove-markup` tool from [Nutrimatic](https://github.com/ebenolson/nutrimatic):
    - `$ pv enwiki-*-pages.articles.xml.bz2 | bzcat | bin/remove-markup | zstd > enwiki-articles.txt.zst`
    - Output file is about ~60% of the original size.
-->


<!--
- Generate basic wordlists + frequencies
    - `$ pv enwiki-*-pages-articles.txt.zst | zstdcat | python3 listgen/listgen.py`
-->
