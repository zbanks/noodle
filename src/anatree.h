#include "prelude.h"
#include "word.h"
#include "wordlist.h"

struct anatree_node {
    size_t n_edges;
    size_t n_words;
    const struct word ** words;
    struct anatree_node ** edge_nodes;
    unsigned char edge_values[0];
};

struct anatree {
    char alphabet[27];
    unsigned char inverse_alphabet[256];
    struct anatree_node * root;
};

NOODLE_EXPORT struct anatree * anatree_create(const struct wordset * ws);
NOODLE_EXPORT void anatree_destory(struct anatree * at);
NOODLE_EXPORT void anatree_node_print(const struct anatree_node * atn);

NOODLE_EXPORT const struct anatree_node * anatree_lookup(const struct anatree * at, const char * s);
