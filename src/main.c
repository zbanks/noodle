#include "libnoodle.h"
#include <regex.h>

int main() {
    nx_test();

    struct word w;
    const char * phrase = "Hello, World!";
    word_init(&w, phrase, strlen(phrase));
    LOG("> \"%s\"", word_str(&w));
    word_term(&w);

    struct wordlist wl;
    // ASSERT(wordlist_init_from_file(&wl, "/usr/share/dict/words") == 0);
    ASSERT(wordlist_init_from_file(&wl, "consolidated.txt") == 0);
    struct wordset * ws = &wl.self_set;

    if (0) {
        // const char * regex = "^\\(test\\|hello\\|as\\|pen\\|world\\|[isdf][isdf]\\|a\\?b\\?c\\?d\\?e\\?\\)\\+$";
        // const char * regex = "^hellt?oworld$";
        // const char * regex = "T?E?R?O?L?K?C?I?L?S?T?G?O?N?C?I?L?B?K?S?M?A?G?T?F?O?D?N?I?K?O?P?G?A?E?E?H?T?H?E?R?C?";
        // const char * regex =
        // "MO?R?E?N?O?S?P?O?O?K?Y?I?S?N?O?E?R?P?A?D?I?G?S?T?U?N?R?I?T?C?E?L?L?O?B?C?I?N?N?O?S?R?G?E?P?";
        // const char *regex = "^\\([asdf][asdf]\\)\\+$";
        // const char * regex = "^h\\(e\\|ow\\)l*o\\?w*[orza]\\+l\\?d*$";
        // const char *regex = "^helloworld$";
        // const char * regex = "^(goodbye|(hellt?o)+)worq?[aild]*d$";
        // const char *regex = "^...$";
        const char * regex = "^h?e?l*o?hello$";
        struct nx * nx = nx_compile(regex, 0);
        int64_t t = now_ns();
        size_t n_matches[32] = {0};
        for (size_t i = 0; i < ws->words_count; i++) {
            const char * s = word_str(ws->words[i]);
            int rc = nx_match(nx, s, 0);
            n_matches[(size_t)(rc + 1)]++;
            // if (rc == 0) LOG("> match: %s", s);
        }
        t = now_ns() - t;
        LOG("> %zu misses; %zu perfect matches; %zu 1-off matches: " PRIlong " ns (" PRIlong " ms; %0.1lf ns/word)",
            n_matches[0], n_matches[1], n_matches[2], t, t / (long)1e6, (double)t / (double)ws->words_count);
        LOG("> [%zu, %zu, %zu, %zu, %zu, %zu, %zu, %zu, ...]", n_matches[0], n_matches[1], n_matches[2], n_matches[3],
            n_matches[4], n_matches[5], n_matches[6], n_matches[7]);

        regex_t preg;
        regcomp(&preg, regex, REG_ICASE | REG_NOSUB);

        t = now_ns();
        size_t n_matches_regexec = 0;
        for (size_t i = 0; i < ws->words_count; i++) {
            const char * s = word_str(ws->words[i]);
            int rc = regexec(&preg, s, 0, NULL, 0);
            if (rc == 0) {
                n_matches_regexec++;
            }
        }
        t = now_ns() - t;
        LOG("Time for regexec evaluation: " PRIlong " ns (" PRIlong " ms)", t, t / (long)1e6);

        size_t n_mismatches = 0;
        for (size_t i = 0; i < ws->words_count; i++) {
            const char * s = word_str(ws->words[i]);
            int rc1 = nx_match(nx, s, 0);
            int rc2 = regexec(&preg, s, 0, NULL, 0);
            if ((rc1 == 0) != (rc2 == 0)) {
                // LOG("Mismatch on \"%s\": nx=%d, regexec=%d", s, rc1, rc2);
                n_mismatches++;
            }
        }
        LOG("# mismatches against regexec: %zu", n_mismatches);
        regfree(&preg);
        nx_destroy(nx);
    }

    {
        struct cursor cursor;
        struct wordlist buffer;
        wordlist_init(&buffer);
        cursor_init_print(&cursor, 100);
        cursor_set_deadline(&cursor, now_ns() + (int64_t)10e9, 1000);

        struct nx * nxs[20] = {0};
        nxs[0] = NONNULL(nx_compile("_..._._..._", NX_FLAG_EXPLICIT_SPACE));
        nxs[1] = NONNULL(nx_compile("[angrm][angrm][angrm][angrm][angrm][angrm][angrm]", 0));
        nxs[2] = NONNULL(nx_compile("[ngrm]*a[ngrm]*a[ngrm]*a[ngrm]*", 0));
        nxs[3] = NONNULL(nx_compile("[agrm]*n[agrm]*", 0));
        nxs[4] = NONNULL(nx_compile("[anrm]*g[anrm]*", 0));
        nxs[5] = NONNULL(nx_compile("[angm]*r[angm]*", 0));
        nxs[6] = NONNULL(nx_compile("[angr]*m[angr]*", 0));
        nxs[7] = NONNULL(nx_compile("a?n?a?g?r?a?m?a?n?a?g?r?a?m", 0));

        do {
            cursor.deadline_output_index++;
            nx_combo_multi(nxs, 8, ws, 3, &cursor);
        } while (cursor.total_input_items != cursor.input_index && now_ns() < cursor.deadline_ns);
        LOG("Multi match: %s", cursor_debug(&cursor));

        for (struct nx ** nx = &nxs[0]; *nx != NULL; nx++) {
            nx_destroy(*nx);
            *nx = NULL;
        }
        wordlist_term(&buffer);
        wordlist_term(&wl);
    }

    return 0;
}
