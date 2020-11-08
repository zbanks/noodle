#include "anatree.h"
#include "filter.h"
#include "nx.h"
#include "prelude.h"
#include "word.h"
#include "wordlist.h"
#include <time.h>

int64_t now() {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec * 1000000000 + t.tv_nsec;
}

int main() {
    nx_test();
    return 0;

    struct word w;
    word_init(&w, "Hello, World!", 10);
    LOG("> %s", word_debug(&w));
    word_term(&w);

    struct wordlist wl;
    // ASSERT(wordlist_init_from_file(&wl, "/usr/share/dict/words", false) == 0);
    ASSERT(wordlist_init_from_file(&wl, "consolidated.txt", true) == 0);
    struct wordset * ws = &wl.self_set;
    LOG("Wordlist: %zu words from %s", ws->words_count, ws->name);
    LOG("wl[1000] = %s", word_debug(ws->words[1000]));
    wordset_sort_value(&wl.self_set);
    LOG("top score = %s", word_debug(ws->words[0]));

    struct anatree * at = anatree_create(ws);
    int64_t start_ns = now();
    const struct anatree_node * atn = anatree_lookup(at, "smiles");
    int64_t end_ns = now();
    anatree_node_print(atn);
    LOG("Lookup in %lu ns", end_ns - start_ns);
    anatree_destory(at);

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

    // struct filter * f1 = NONNULL(filter_parse("extract: ab(.{7})"));
    struct filter * f1 = NONNULL(filter_parse("superanagram: eeeeeeeee"));
    struct filter * f2 = NONNULL(filter_parse("extractq: .(.*)."));
    // struct filter * f3 = NONNULL(filter_parse("anagram: .*e(..).*"));
    struct wordset wso;
    wordset_init(&wso, "filter matches");
    filter_chain_apply((struct filter * const[]){f1, f2}, 1, ws, &wso, &buffer);
    wordset_print(&wso);

    struct word wt;
    word_tuple_init(&wt, wso.words, 3);
    LOG("wordtuple: %s", word_debug(&wt));
    word_term(&wt);

    wordset_term(&wso);
    filter_destroy(f1);
    filter_destroy(f2);

    wordlist_term(&buffer);
    wordlist_term(&wl);
    return 0;
}
