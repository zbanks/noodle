#include "libnoodle.h"
#include <regex.h>

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
    const char * regex = "^h?e?l*o?hello$";
    // const char * regex = "^(goodbye|(hellt?o)+)worq?[aild]*d$";
    // const char *regex = "^...$";
    struct nx * nx = nx_compile(regex);
    int64_t t = now_ns();
    size_t n_matches[32] = {0};
    for (size_t i = 0; i < ws->words_count; i++) {
        const char * s = word_canonical(ws->words[i]);
        int rc = nx_match(nx, s, 0);
        n_matches[(size_t)(rc + 1)]++;
        // if (rc == 0) LOG("> match: %s", s);
    }
    t = now_ns() - t;
    LOG("> %zu misses; %zu perfect matches; %zu 1-off matches: %ld ns (%ld ms; %0.1lf ns/word)", n_matches[0],
        n_matches[1], n_matches[2], t, t / (long)1e6, (double)t / (double)ws->words_count);
    LOG("> [%zu, %zu, %zu, %zu, %zu, %zu, %zu, %zu, ...]", n_matches[0], n_matches[1], n_matches[2], n_matches[3],
        n_matches[4], n_matches[5], n_matches[6], n_matches[7]);

    regex_t preg;
    regcomp(&preg, regex, REG_ICASE | REG_NOSUB);

    t = now_ns();
    size_t n_matches_regexec = 0;
    for (size_t i = 0; i < ws->words_count; i++) {
        const char * s = word_canonical(ws->words[i]);
        int rc = regexec(&preg, s, 0, NULL, 0);
        if (rc == 0) {
            n_matches_regexec++;
        }
    }
    t = now_ns() - t;
    LOG("Time for regexec evaluation: %ld ns (%ld ms)", t, t / (long)1e6);

    size_t n_mismatches = 0;
    for (size_t i = 0; i < ws->words_count; i++) {
        const char * s = word_canonical(ws->words[i]);
        int rc1 = nx_match(nx, s, 0);
        int rc2 = regexec(&preg, s, 0, NULL, 0);
        if ((rc1 == 0) != (rc2 == 0)) {
            // LOG("Mismatch on \"%s\": nx=%d, regexec=%d", s, rc1, rc2);
            n_mismatches++;
        }
    }
    LOG("# mismatches against regexec: %zu", n_mismatches);
    regfree(&preg);

    // XXX: I've gotten sloppy with my resource management here; there's some leaks
    struct cursor cursor;
    struct wordlist buffer;
    wordlist_init(&buffer, "buffer");
    if (0) {
        cursor_init(&cursor);
        cursor_set_deadline(&cursor, now_ns() + (int64_t)10e9, 1000000);
        struct filter * fnxn = NONNULL(filter_create(FILTER_NXN, 2, regex));
        // cursor_set_deadline(&cursor, 0, 0);
        struct word_callback * cb = word_callback_create_print(&cursor, 0);
        do {
            cursor.deadline_output_index++;
            filter_chain_apply((const struct filter * const[]){fnxn}, 1, ws, &cursor, cb);
            // LOG("%zu %zu %lu", cursor.total_input_items, cursor.input_index, cursor.deadline_ns);
        } while (cursor.total_input_items != cursor.input_index && now_ns() < cursor.deadline_ns);
        free(cb);
        LOG("Combo match: %s", cursor_debug(&cursor));
        filter_destroy(fnxn);
        nx_destroy(nx);
    }

    nx_test();
    {
        cursor_init(&cursor);
        cursor_set_deadline(&cursor, now_ns() + (int64_t)10e9, 1000);
        struct word_callback * cb = word_callback_create_print(&cursor, 0);

        struct nx * nxs[8] = {0};
        // nxs[0] = NONNULL(nx_compile("h?e?l*o?z*w?o?q*r?l?d?"));
        // nxs[1] = NONNULL(nx_compile(".........."));
        // nxs[2] = NONNULL(nx_compile(".*l.*l.*l.*"));

        // nxs[0] = NONNULL(nx_compile("hello.*world"));

        /*
[angrm][angrm][angrm][angrm][angrm][angrm][angrm]
[ngrm]*a[ngrm]*a[ngrm]*a[ngrm]*
[agrm]*n[agrm]*
[anrm]*g[anrm]*
[angm]*r[angm]*
[angr]*m[angr]*
a?n?a?g?r?a?m?a?n?a?g?r?a?m
_..._._..._
        */

        nxs[0] = NONNULL(nx_compile("[angrm][angrm][angrm][angrm][angrm][angrm][angrm]"));
        nxs[1] = NONNULL(nx_compile("[ngrm]*a[ngrm]*a[ngrm]*a[ngrm]*"));
        nxs[2] = NONNULL(nx_compile("[agrm]*n[agrm]*"));
        nxs[3] = NONNULL(nx_compile("[anrm]*g[anrm]*"));
        nxs[4] = NONNULL(nx_compile("[angm]*r[angm]*"));
        nxs[5] = NONNULL(nx_compile("[angr]*m[angr]*"));
        nxs[6] = NONNULL(nx_compile("a?n?a?g?r?a?m?a?n?a?g?r?a?m"));
        nxs[7] = NONNULL(nx_compile("_..._._..._"));
        // nxs[6] = NONNULL(nx_compile("gram.*"));

        // nxs[0] = NONNULL(nx_compile(".........."));
        // nxs[1] = NONNULL(nx_compile(".*a.*a.*a.*"));
        // nxs[2] = NONNULL(nx_compile(".*n.*"));
        // nxs[3] = NONNULL(nx_compile(".*g.*"));
        // nxs[4] = NONNULL(nx_compile(".*r.*"));
        // nxs[5] = NONNULL(nx_compile(".*m.*"));
        // nxs[6] = NONNULL(nx_compile("gram.*"));

        do {
            cursor.deadline_output_index++;
            nx_combo_multi(nxs, 8, ws, 3, &cursor, cb);
        } while (cursor.total_input_items != cursor.input_index && now_ns() < cursor.deadline_ns);
        LOG("Multi match: %s", cursor_debug(&cursor));
        return 0;
    }

    {
        wordset_print(ws);
        struct cursor;
        cursor_init(&cursor);
        cursor_set_deadline(&cursor, now_ns() + (int)1e9, 0);
        struct word_callback * cb = word_callback_create_print(&cursor, 0);
        do {
            cursor.deadline_output_index++;
            anagram_slow(ws, "aaii", &cursor, cb);
        } while (cursor.total_input_items != cursor.input_index && now_ns() < cursor.deadline_ns);
    }
    return 0;

    int64_t start_ns = now_ns();
    struct anatree * at = anatree_create(ws);
    LOG("created anatree for %zu words in %ld ns", ws->words_count, now_ns() - start_ns);
    start_ns = now_ns();
    const struct anatree_node * atn = anatree_lookup(at, "smiles");
    int64_t end_ns = now_ns();
    anatree_node_print(atn);
    LOG("Lookup in %lu ns", end_ns - start_ns);
    anatree_anagrams(at, "trains", NULL, NULL);
    anatree_destory(at);
    return 0;

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
    struct filter * f4 = NONNULL(filter_parse("score 10:"));
    // struct filter * f4 = NONNULL(filter_parse("anagram: .*e(..).*"));
    cursor_init(&cursor);
    cursor_set_deadline(&cursor, now_ns() + (int)1e9, 0);
    struct wordset wso;
    wordset_init(&wso, "filter matches");
    struct word_callback * cb_buffer = word_callback_create_wordset_add(&cursor, &buffer, &wso);
    do {
        cursor.deadline_output_index++;
        filter_chain_apply((const struct filter * const[]){f1, f2, f3, f4}, 4, ws, &cursor, cb_buffer);
        LOG("Cursor state: %s", cursor_debug(&cursor));
    } while (cursor.input_index != cursor.total_input_items);
    free(cb_buffer);
    wordset_print(&wso);

    wordset_term(&wso);
    wordset_init(&wso, "anagrams of spears via 6 nx");
    const struct filter * fanagram[6] = {
        NONNULL(filter_parse("nx: [spear][spear][spear][spear][spear][spear]")),
        NONNULL(filter_parse("nx: [^s]*s[^s]*s[^s]*")),
        NONNULL(filter_parse("nx: [^p]*p[^p]*")),
        NONNULL(filter_parse("nx: [^e]*e[^e]*")),
        NONNULL(filter_parse("nx: [^a]*a[^a]*")),
        NONNULL(filter_parse("nx: [^r]*r[^r]*")),
    };
    cursor_init(&cursor);
    cursor_set_deadline(&cursor, now_ns() + (int)1e9, 0);
    struct word_callback * cb = word_callback_create_print(&cursor, 0);
    filter_chain_apply(fanagram, 6, ws, &cursor, cb);
    free(cb);
    LOG("Cursor state: %s", cursor_debug(&cursor));
    wordset_print(&wso);

    wordset_term(&wso);
    wordset_init(&wso, "anagrams of spears via anagram filter");
    const struct filter * fanagram2 = NONNULL(filter_parse("anagram: spears"));
    cursor_init(&cursor);
    cursor_set_deadline(&cursor, now_ns() + (int)1e9, 0);
    cb = word_callback_create_print(&cursor, 3);
    filter_chain_apply(&fanagram2, 1, ws, &cursor, cb);
    free(cb);
    LOG("Cursor state: %s", cursor_debug(&cursor));
    wordset_print(&wso);

    // struct word wt;
    // word_tuple_init(&wt, wso.words, 3);
    // LOG("wordtuple: %s", word_debug(&wt));
    // word_term(&wt);

    wordset_term(&wso);
    filter_destroy(f1);
    filter_destroy(f2);

    wordlist_term(&buffer);
    wordlist_term(&wl);
    return 0;
}
