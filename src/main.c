#include "anatree.h"
#include "filter.h"
#include "nx.h"
#include "nx_combo.h"
#include "prelude.h"
#include "word.h"
#include "wordlist.h"
#include <regex.h>
#include <time.h>

int64_t now() {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec * 1000000000 + t.tv_nsec;
}

int main() {
    nx_test();

    struct word w;
    word_init(&w, "Hello, World!", 10);
    LOG("> %s", word_debug(&w));
    word_term(&w);

    struct wordlist wl;
    ASSERT(wordlist_init_from_file(&wl, "/usr/share/dict/words", false) == 0);
    // ASSERT(wordlist_init_from_file(&wl, "consolidated.txt", true) == 0);
    struct wordset * ws = &wl.self_set;
    /*
    LOG("Wordlist: %zu words from %s", ws->words_count, ws->name);
    LOG("wl[1000] = %s", word_debug(ws->words[1000]));
    wordset_sort_value(&wl.self_set);
    LOG("top score = %s", word_debug(ws->words[0]));
    */

    // const char * regex = "^\\(test\\|hello\\|as\\|pen\\|world\\|[isdf][isdf]\\|a\\?b\\?c\\?d\\?e\\?\\)\\+$";
    // const char * regex = "^hellt?oworld$";
    // const char * regex = "T?E?R?O?L?K?C?I?L?S?T?G?O?N?C?I?L?B?K?S?M?A?G?T?F?O?D?N?I?K?O?P?G?A?E?E?H?T?H?E?R?C?";
    // const char * regex =
    // "MO?R?E?N?O?S?P?O?O?K?Y?I?S?N?O?E?R?P?A?D?I?G?S?T?U?N?R?I?T?C?E?L?L?O?B?C?I?N?N?O?S?R?G?E?P?";
    // const char *regex = "^\\([asdf][asdf]\\)\\+$";
    // const char * regex = "^h\\(e\\|ow\\)l*o\\?w*[orza]\\+l\\?d*$";
    // const char *regex = "^helloworld$";
    const char * regex = "^(goodbye|(hellt?o)+)worq?[aild]*d$";
    struct nx * nx = nx_compile(regex);
    int64_t t = now();
    size_t n_matches[32] = {0};
    for (size_t i = 0; i < ws->words_count; i++) {
        const char * s = str_str(&ws->words[i]->canonical);
        int rc = nx_match(nx, s, 0);
        n_matches[(size_t)(rc + 1)]++;
        // if (rc == 0) LOG("> match: %s", s);
    }
    t = now() - t;
    LOG("> %zu misses; %zu perfect matches; %zu 1-off matches: %ld ns (%ld ms; %0.1lf ns/word)", n_matches[0],
        n_matches[1], n_matches[2], t, t / (long)1e6, (double)t / (double)ws->words_count);
    LOG("> [%zu, %zu, %zu, %zu, %zu, %zu, %zu, %zu, ...]", n_matches[0], n_matches[1], n_matches[2], n_matches[3],
        n_matches[4], n_matches[5], n_matches[6], n_matches[7]);

    regex_t preg;
    regcomp(&preg, regex, REG_ICASE | REG_NOSUB);

    t = now();
    size_t n_matches_regexec = 0;
    for (size_t i = 0; i < ws->words_count; i++) {
        const char * s = str_str(&ws->words[i]->canonical);
        int rc = regexec(&preg, s, 0, NULL, 0);
        if (rc == 0) {
            n_matches_regexec++;
        }
    }
    t = now() - t;
    LOG("Time for regexec evaluation: %ld ns (%ld ms)", t, t / (long)1e6);

    size_t n_mismatches = 0;
    for (size_t i = 0; i < ws->words_count; i++) {
        const char * s = str_str(&ws->words[i]->canonical);
        int rc1 = nx_match(nx, s, 0);
        int rc2 = regexec(&preg, s, 0, NULL, 0);
        if ((rc1 == 0) != (rc2 == 0)) {
            // LOG("Mismatch on \"%s\": nx=%d, regexec=%d", s, rc1, rc2);
            n_mismatches++;
        }
    }
    LOG("# mismatches against regexec: %zu", n_mismatches);
    regfree(&preg);

    // I've gotten sloppy with my resource management here; there's some leaks
    struct wordlist buffer;
    wordlist_init(&buffer, "buffer");
    struct wordset combo_ws;
    wordset_init(&combo_ws, "combo matches");
    t = now();
    int rc = nx_combo_match(nx, ws, 3, &combo_ws, &buffer);
    t = now() - t;
    LOG("Combo match found %zu matches (rc = %d) in %ld ns (%ld ms)", combo_ws.words_count, rc, t, t / (long)1e6);

    nx_destroy(nx);
    // return 0;

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

    struct filter * f1 = NONNULL(filter_parse("extract: ab(.{7})"));
    // struct filter * f1 = NONNULL(filter_parse("superanagram: eeee"));
    struct filter * f2 = NONNULL(filter_parse("extractq: .(.*)."));
    struct filter * f3 = NONNULL(filter_parse("nx 1: .*in"));
    // struct filter * f4 = NONNULL(filter_parse("anagram: .*e(..).*"));
    struct wordset wso;
    wordset_init(&wso, "filter matches");
    filter_chain_apply((struct filter * const[]){f1, f2, f3}, 3, ws, &wso, &buffer);
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
