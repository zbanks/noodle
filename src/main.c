#include "filter.h"
#include "prelude.h"
#include "word.h"
#include "wordlist.h"

int main() {
    struct word w;
    word_init(&w, "Hello, World!", 10);
    LOG("> " PRIWORD, PRIWORDF(w));
    word_term(&w);

    struct wordlist wl;
    ASSERT(wordlist_init_from_file(&wl, "/usr/share/dict/words") == 0);
    struct wordset * ws = &wl.self_set;
    LOG("Wordlist: %zu words from %s", ws->words_count, ws->name);
    LOG("wl[1000] = " PRIWORD, PRIWORDF(*ws->words[1000]));
    wordset_sort_value(&wl.self_set);
    LOG("top score = " PRIWORD, PRIWORDF(*ws->words[0]));

    struct wordset regex_matches;
    wordset_init(&regex_matches, ".a.io matches");
    ASSERT(filter_regex(".a.io.*", ws, &regex_matches) == 0);
    LOG("top score for regex = " PRIWORD, PRIWORDF(*regex_matches.words[0]));
    wordset_term(&regex_matches);

    // struct filter f = {.type = FILTER_BANK, .arg_str = "aksdlfe", .arg_n = 0};
    struct filter f;
    // ASSERT(filter_parse(&f, "transadd 1: asdf") == 0);
    filter_init(&f, FILTER_BANK, 0, "asdfklez");
    ASSERT(filter_apply(&f, ws) == 0);
    LOG("top score for filter '%s' = " PRIWORD, f.name, PRIWORDF(*f.output.words[0]));
    filter_term(&f);

    wordlist_term(&wl);
    return 0;
}
