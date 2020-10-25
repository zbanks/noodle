#include "anatree.h"

struct anatree_hist {
    bool invalid;
    unsigned char h[31];
    const struct word * word;
};

int anatree_hist_cmp(const void * a, const void * b) { return memcmp(a, b, sizeof(struct anatree_hist)); }

static void anatree_histogram(const struct anatree * at, const char * s, struct anatree_hist * ath_out) {
    unsigned char counts[26];
    memset(counts, 0, sizeof(counts));
    for (; *s != '\0'; s++) {
        unsigned char idx = at->inverse_alphabet[(unsigned char)*s];
        counts[idx]++;
        if (counts[idx] == 255) {
            ath_out->invalid = true;
            return;
        }
    }
    memset(ath_out, 0, sizeof(*ath_out));
    unsigned char * h = ath_out->h;
    unsigned char * h_end = &ath_out->h[sizeof(ath_out->h)];
    for (size_t i = 0; i < 26; i++) {
        if (counts[i] == 0) {
            continue;
        }
        while (counts[i]) {
            unsigned char d = (unsigned char)MIN(counts[i], 9);
            counts[i] = (unsigned char)(counts[i] - d);
            *h++ = (unsigned char)((((unsigned char)(i + 1)) << 3) | (d - 1));
        }
        if (h == h_end) {
            memset(ath_out, 0, sizeof(*ath_out));
            ath_out->invalid = true;
            return;
        }
    }
}

struct anatree_node * anatree_construct(struct anatree_hist * hists, size_t n_hists, size_t depth) {
    size_t n_words = 0;
    size_t n_edges = 0;
    unsigned char last = 0;
    for (size_t i = 0; i < n_hists; i++) {
        ASSERT(!hists[i].invalid);
        unsigned char h = hists[i].h[depth];
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
    atn->words = NONNULL(calloc(n_words, sizeof(*atn->words)));
    atn->edge_nodes = NONNULL(calloc(n_edges, sizeof(*atn->edge_nodes)));

    size_t word_index = 0;
    size_t edge_index = 0;
    for (size_t i = 0; i < n_hists;) {
        unsigned char h = hists[i].h[depth];
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
        atn->edge_nodes[edge_index] = anatree_construct(&hists[i], n_at_edge, depth + 1);
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
        const unsigned char a = (unsigned char)at->alphabet[i];
        ASSERT(at->inverse_alphabet[a] == 0xFF);
        at->inverse_alphabet[a] = (unsigned char)i;
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

    at->root = anatree_construct(histograms, valid_count, 0);
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
