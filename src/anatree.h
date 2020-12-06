#pragma once

#include "prelude.h"
#include "time_util.h"
#include "word.h"
#include "wordlist.h"

struct anatree_node {
    size_t n_edges;
    size_t n_words;
    const struct word ** words;
    struct anatree_node ** edge_nodes;
    char label[64]; // XXX: This can be removed in the final datastructure; only used for debugging
    unsigned char edge_values[0];
};

struct anatree {
    char alphabet[27];
    unsigned char inverse_alphabet[256];
    struct anatree_node * root;
};

// Constructing the anatree usually takes ~1ms per 1000 words (but is technically O(n log n)ish)
NOODLE_EXPORT struct anatree * anatree_create(const struct wordset * ws);
NOODLE_EXPORT void anatree_destory(struct anatree * at);
NOODLE_EXPORT void anatree_node_print(const struct anatree_node * atn);

// XXX: These two functions are in-progress/broken
NOODLE_EXPORT const struct anatree_node * anatree_lookup(const struct anatree * at, const char * s);

NOODLE_EXPORT void anatree_anagrams(const struct anatree * at, const char * s, struct cursor * cursor,
                                    struct word_callback * cb);
