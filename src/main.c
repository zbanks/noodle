#include "filter.h"
#include "prelude.h"
#include "word.h"
#include "wordlist.h"

int main() {
    struct word w;
    word_init(&w, "Hello, World!", 10);
    LOG("> %s", word_debug(&w));
    word_term(&w);

    struct wordlist wl;
    ASSERT(wordlist_init_from_file(&wl, "/usr/share/dict/words") == 0);
    struct wordset * ws = &wl.self_set;
    LOG("Wordlist: %zu words from %s", ws->words_count, ws->name);
    LOG("wl[1000] = %s", word_debug(ws->words[1000]));
    wordset_sort_value(&wl.self_set);
    LOG("top score = %s", word_debug(ws->words[0]));

    /*
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
    */
    struct wordlist buffer;
    wordlist_init(&buffer, "buffer");

    struct filter * f1 = NONNULL(filter_parse("extract: ab(.{7})"));
    //struct filter * f2 = NONNULL(filter_parse("extractq: .*e(..).*"));
    struct filter * f2 = NONNULL(filter_parse("extractq: ...(.*)"));
    struct wordset wso;
    wordset_init(&wso, "filter matches");
    filter_chain_apply((struct filter * const []){f1, f2}, 2, ws, &wso, &buffer);
    wordset_print(&wso);

    struct word wt;
    word_tuple_init(&wt, wso.words, 3);
    LOG("wordtuple: %s", word_debug(&wt));
    word_term(&wt);

    wordset_term(&wso);
    filter_destroy(f1);
    filter_destroy(f2);

    wordlist_term(&wl);
    return 0;
}
