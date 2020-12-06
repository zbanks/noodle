#include "anatree.h"
#include <stddef.h>

struct anatree_hist {
    bool invalid;
    uint8_t h[31];
    const struct word * word;
};

int anatree_hist_cmp(const void * a, const void * b) { return memcmp(a, b, sizeof(struct anatree_hist)); }

static const char * anatree_hist_debug(const struct anatree * at, const uint8_t * h) {
    static char buffer[1024];
    char * b = buffer;
    while (*h != 0) {
        b += sprintf(b, "%c%d ", at->alphabet[*h / 8 - 1], (*h & 7) + 1);
        h++;
    }
    return buffer;
}

static void anatree_histogram(const struct anatree * at, const char * s, struct anatree_hist * ath_out) {
    uint8_t counts[26];
    memset(counts, 0, sizeof(counts));
    for (; *s != '\0'; s++) {
        uint8_t idx = at->inverse_alphabet[(uint8_t)*s];
        counts[idx]++;
        if (counts[idx] == 255) {
            ath_out->invalid = true;
            return;
        }
    }
    memset(ath_out, 0, sizeof(*ath_out));
    uint8_t * h = ath_out->h;
    uint8_t * h_end = &ath_out->h[sizeof(ath_out->h)];
    for (size_t i = 0; i < 26; i++) {
        if (counts[i] == 0) {
            continue;
        }
        while (counts[i]) {
            uint8_t d = (uint8_t)MIN(counts[i], 8);
            counts[i] = (uint8_t)(counts[i] - d);
            *h++ = (uint8_t)((((uint8_t)(i + 1)) << 3) | (d - 1));
        }
        if (h == h_end) {
            memset(ath_out, 0, sizeof(*ath_out));
            ath_out->invalid = true;
            return;
        }
    }
}

struct anatree_node * anatree_construct(struct anatree_hist * hists, size_t n_hists, size_t depth, const char * label,
                                        const char * alphabet) {
    size_t n_words = 0;
    size_t n_edges = 0;
    uint8_t last = 0;
    for (size_t i = 0; i < n_hists; i++) {
        ASSERT(!hists[i].invalid);
        uint8_t h = hists[i].h[depth];
        if (h == 0) {
            n_words++;
            continue;
        }
        if (h == last) {
            continue;
        }
        last = h;
        n_edges++;
    }

    struct anatree_node * atn = NONNULL(calloc(1, sizeof(struct anatree_node) + n_edges * sizeof(atn->edge_nodes[0])));
    atn->n_edges = n_edges;
    atn->n_words = n_words;
    atn->words = NONNULL(calloc(n_words + 1, sizeof(*atn->words)));
    atn->edge_nodes = NONNULL(calloc(n_edges + 1, sizeof(*atn->edge_nodes)));
    strcpy(atn->label, label);

    size_t word_index = 0;
    size_t edge_index = 0;
    for (size_t i = 0; i < n_hists;) {
        uint8_t h = hists[i].h[depth];
        if (h == 0) {
            atn->words[word_index++] = hists[i].word;
            i++;
            continue;
        }
        size_t n_at_edge = 1;
        while (hists[i + n_at_edge].h[depth] == h && (i + n_at_edge) < n_hists) {
            n_at_edge++;
        }
        atn->edge_values[edge_index] = h;
        char buffer[32] = {0};
        for (size_t i = 0; i < (h % 8u) + 1u; i++) {
            buffer[i] = alphabet[(h / 8u) - 1u];
        }
        char node_label[64] = {0};
        strcpy(node_label, atn->label);
        strcat(node_label, buffer);
        atn->edge_nodes[edge_index] = NONNULL(anatree_construct(&hists[i], n_at_edge, depth + 1, node_label, alphabet));
        edge_index++;
        i += n_at_edge;
    }
    ASSERT(word_index == n_words);
    ASSERT(edge_index == n_edges);
    qsort(atn->words, n_words, sizeof(*atn->words), word_value_ptrcmp);
    return atn;
}

int cmp_size(const void * _a, const void * _b) {
    const size_t * a = _a;
    const size_t * b = _b;
    if (*a < *b)
        return -1;
    if (*a > *b)
        return 1;
    return 0;
}

struct anatree * anatree_create(const struct wordset * ws) {
    struct anatree * at = NONNULL(calloc(1, sizeof(*at)));
    //*at = (struct anatree) {
    //    .alphabet = "etaoinshrdlcumwfgypbvkjxqz",
    //};

    size_t distribution[26][16] = {0};
    for (size_t i = 0; i < ws->words_count; i++) {
        size_t word_distribution[26] = {0};
        for (const char * c = str_str(&ws->words[i]->canonical); *c != '\0'; c++) {
            ASSERT(*c >= 'a' && *c <= 'z');
            word_distribution[(*c - 'a')]++;
        }
        for (size_t j = 0; j < 26; j++) {
            word_distribution[j] = MIN(word_distribution[j], 15u);
            distribution[j][word_distribution[j]]++;
        }
    }
    struct {
        size_t count;
        char letter;
    } max_buckets[26] = {0};
    for (size_t i = 0; i < 26; i++) {
        max_buckets[i].letter = (char)('a' + (char)i);
        max_buckets[i].count = 0;
        for (size_t j = 0; j < 12; j++) {
            max_buckets[i].count = MAX(max_buckets[i].count, distribution[i][j]);
        }
    }
    qsort(max_buckets, 26, sizeof(*max_buckets), cmp_size);
    for (size_t i = 0; i < 26; i++) {
        at->alphabet[i] = max_buckets[i].letter;
    }

    memset(at->inverse_alphabet, 0xFF, sizeof(at->inverse_alphabet));
    for (size_t i = 0; at->alphabet[i] != '\0' && i < sizeof(at->alphabet); i++) {
        const uint8_t a = (uint8_t)at->alphabet[i];
        ASSERT(at->inverse_alphabet[a] == 0xFF);
        at->inverse_alphabet[a] = (uint8_t)i;
    }

    LOG("Starting to load histograms with alphabet: %s", at->alphabet);
    struct anatree_hist * histograms = NONNULL(calloc(ws->words_count, sizeof(*histograms)));
    size_t valid_count = 0;
    for (size_t i = 0; i < ws->words_count; i++) {
        anatree_histogram(at, str_str(&ws->words[i]->canonical), &histograms[i]);
        if (!histograms[i].invalid) {
            valid_count++;
        }
        histograms[i].word = ws->words[i];
    }
    LOG("Loaded %zu valid histograms (out of %zu words)", valid_count, ws->words_count);

    qsort(histograms, ws->words_count, sizeof(*histograms), anatree_hist_cmp);
    ASSERT(!histograms[valid_count - 1].invalid);
    ASSERT(valid_count == ws->words_count || histograms[valid_count].invalid);
    LOG("Sorted histograms");

    at->root = anatree_construct(histograms, valid_count, 0, "", at->alphabet);
    LOG("Constructed anatree");

    return at;
}

static void anatree_node_destroy(struct anatree_node * atn) {
    if (atn == NULL) {
        return;
    }
    for (size_t i = 0; i < atn->n_edges; i++) {
        anatree_node_destroy(atn->edge_nodes[i]);
    }
    free(atn->words);
    free(atn->edge_nodes);
    free(atn);
}

void anatree_destory(struct anatree * at) {
    if (at == NULL || at->root == NULL) {
        return;
    }
    anatree_node_destroy(at->root);
}

const struct anatree_node * anatree_lookup(const struct anatree * at, const char * s) {
    struct anatree_hist hist;
    anatree_histogram(at, s, &hist);
    if (hist.invalid) {
        return NULL;
    }
    struct anatree_node * atn = at->root;
    for (size_t i = 0; i < sizeof(hist.h); i++) {
        if (hist.h[i] == 0) {
            break;
        }
        for (size_t j = 0; j < atn->n_edges; j++) {
            if (atn->edge_values[j] < hist.h[i]) {
                continue;
            }
            if (atn->edge_values[j] > hist.h[i]) {
                return NULL;
            }
            ASSERT(atn->edge_values[j] == hist.h[i]);
            atn = atn->edge_nodes[j];
            break;
        }
    }
    return atn;
}

void anatree_node_print(const struct anatree_node * atn) {
    for (size_t i = 0; i < atn->n_words; i++) {
        LOG(" > %s", word_debug(atn->words[i]));
    }
}

//
// XXX: The remaining parts of this file are in-progress/broken
//

void hist_subtract(const uint8_t * h_in, uint8_t v, uint8_t * h_out) {
    if (*h_in != v) {
        ASSERT(v < *h_in);
        uint8_t c1 = v & 0xF8;
        uint8_t d1 = (uint8_t)((v & 0x07) + 1u);
        uint8_t c2 = *h_in & 0xF8;
        uint8_t d2 = (uint8_t)((*h_in & 0x07) + 1u);
        ASSERT(c1 == c2);
        ASSERT(d1 < d2);
        *h_out++ = (uint8_t)(c1 | (d2 - d1 - 1));
    }
    h_in++;
    while (*h_in != 0) {
        *h_out++ = *h_in++;
    }
    *h_out = 0;
}

void anatree_anagrams_iter(const struct anatree * at, const uint8_t * h, struct cursor * cursor,
                           const struct word ** wordstack, size_t index, const struct anatree_node * start_node,
                           struct word_callback * cb) {
    (void)cursor;
    (void)cb;
    (void)anatree_hist_debug;

    char DEPTH[] = "> > > > > > > > > > > > > > > ";
    DEPTH[(index + 1) * 2] = '\0';

    // LOG("%s iter: %s", DEPTH, anatree_hist_debug(at, h));

    const struct anatree_node * atn = start_node;
    if (atn == NULL) {
        atn = at->root;
    }
    uint8_t subh[32] = {0};
    for (size_t i = 0; h[i] != 0; i++) {
        subh[i] = h[i];
    }

    /*
    struct {
        const struct anatree_node * atn;
        size_t j;
        uint8_t subh[32];
    } stack[32] = {0};
    */

    for (size_t i = 0;; i++) {
        if (h[i] == 0) {
            break;
        }
        // const struct anatree_node * next_atn = NULL;
        // LOG("%s node %s '%s': h_left='%s' i=%zu index=%zu w[0]=%s w[1]=%s", DEPTH, atn->label,
        // word_canonical(atn->words[0]), anatree_hist_debug(at, &h[i]), i, index, word_canonical(wordstack[0]),
        // word_canonical(wordstack[1]));
        for (size_t j = 0; j < atn->n_edges; j++) {
            if (atn->edge_values[j] > h[i]) {
                return;
            }
            if ((atn->edge_values[j] ^ h[i]) & 0xF8) {
                continue;
            }
            /*
            if (atn->edge_values[j] < h[i]) {
                stack[0].atn = atn->edge_nodes[j];
                stack[0].j = 0;
                hist_subtract(&h[i], atn->edge_values[j], stack[0].subh);
                size_t s = 0;
                while (true) {
                    for (size_t sj = 0; sj < stack[s].atn->n_edges; sj++) {
                        stack[s].atn
                    }
                }
            }
            */

            hist_subtract(&h[i], atn->edge_values[j], subh);
            if (true && index == 0) {
                // LOG("%s iter push: %s [%c]: %s", DEPTH, atn->edge_nodes[j]->label, at->alphabet[h[i] /8 - 1],
                // anatree_hist_debug(at, subh));
                // LOG("was: %s (i=%zu, j=%zu)", anatree_hist_debug(at, h), i, j);
            }
            if (atn->words[0] != NULL) {
                wordstack[index] = atn->words[0];
                LOG("hi  ='%s'", anatree_hist_debug(at, &h[i]));
                LOG("subh='%s'", anatree_hist_debug(at, subh));
                anatree_anagrams_iter(at, &h[i], cursor, wordstack, index + 1, NULL, cb);
                wordstack[index] = NULL;
            }

            // LOG("%s iter push: %s [%c]: %s", ">>>>>>>>>"+8-index, word_canonical(atn->words[0]), at->alphabet[h[i] /8
            // - 1], anatree_hist_debug(at, subh));
            // anatree_anagrams_iter(at, subh, cursor, wordstack, index+1, cb);
            // LOG("%s iter pop : %s [%c]", "<<<<<<<<<"+8-index, word_canonical(atn->words[0]), at->alphabet[h[i] /8 -
            // 1]);

            if (atn->edge_values[j] < h[i]) {
                ASSERT(0);
                // anatree_anagrams_iter(at, &h[i], cursor, wordstack, index, atn->edge_nodes[j], cb);
                continue;
            }
            if (atn->words[0] != NULL) {
                // LOG("%s intermediate: %s", DEPTH, word_canonical(atn->words[0]));
            }
            ASSERT(atn->edge_values[j] == h[i]);
            atn = atn->edge_nodes[j];
            break;
        }
    }
    if (atn->words[0] != NULL && start_node == NULL) {
        wordstack[index] = atn->words[0];
        // anatree_anagrams_iter(at, subh, cursor, wordstack, index+1, cb);
        LOG("%s final: %s, %s, %s, %s, %s...", DEPTH, word_canonical(wordstack[0]), word_canonical(wordstack[1]),
            word_canonical(wordstack[2]), word_canonical(wordstack[3]), word_canonical(wordstack[4]));
        wordstack[index] = NULL;
    } else {
        // LOG("%s return index %zu", DEPTH, index);
    }
}

void anatree_anagrams(const struct anatree * at, const char * s, struct cursor * cursor, struct word_callback * cb) {
    struct anatree_hist hist;
    anatree_histogram(at, s, &hist);
    if (hist.invalid) {
        return;
    }
    ASSERT(hist.h[30] == '\0');

    const struct word * wordstack[16] = {0};
    LOG("alphabet: %s", at->alphabet);
    LOG("starting with %s: %s", s, anatree_hist_debug(at, hist.h));

    anatree_anagrams_iter(at, hist.h, cursor, wordstack, 0, NULL, cb);
}
