#pragma once

#include "prelude.h"

#define NX_SET_SIZE ((size_t)256)
#define NX_BRANCH_COUNT ((size_t)2)

#define NX_SET_ARRAYLEN ((size_t)4)
//#define NX_SET_ARRAYLEN ((NX_SET_SIZE + 63) / 64)
_Static_assert(NX_SET_ARRAYLEN == ((NX_SET_SIZE + 63) / 64), "Invalid NX_SET_ARRAYLEN");

struct nx_set {
    uint64_t xs[NX_SET_ARRAYLEN];
};
bool nx_set_isempty(const struct nx_set * s);
bool nx_set_test(const struct nx_set * s, size_t i);
bool nx_set_add(struct nx_set * s, size_t i);

enum nx_char {
    NX_CHAR_END = 0,
    NX_CHAR_EPSILON,
    NX_CHAR_INVALID,
    NX_CHAR_SPACE,

    NX_CHAR_A,
    NX_CHAR_Z = NX_CHAR_A + 25,

    _NX_CHAR_MAX,
};
void nx_char_translate(const char * input, enum nx_char * output, size_t output_size);

struct nx_state {
    enum {
        STATE_TYPE_TRANSITION,
        // STATE_ANAGRAM_EXACT,
        // STATE_ANAGRAM_LIMIT,
    } type;
    union {
        struct {
            // This representation is optimized to be performant when
            // evaluating "unoptimized" NFAs, e.g. the results of
            // Thompson's Construction.
            //
            // Although this *could* represent the DFA form, a DFA
            // would have significantly more branching, and would probably
            // be better represented as a lookup table.
            //
            // This form is also very condusive to "fuzzy" matching
            uint16_t next_state[NX_BRANCH_COUNT];
            uint32_t char_bitset[NX_BRANCH_COUNT];

            // The set of states reachable from this state through epsilon
            // transitions is pre-computed, so that the NFA can be
            // evalutated in linear time.
            struct nx_set epsilon_states;
        };
        // struct {
        //    uint16_t transition_fail;
        //    uint16_t transition_success;
        //    int16_t anagram_arg;
        //    uint8_t anagram_letters[(_NX_CHAR_MAX - 4) * 2];
        //};
    };
};

#define NX_STATE_MAX ((size_t)254)
//#define NX_STATE_MAX (NX_SET_SIZE - 2)
_Static_assert(NX_STATE_MAX == (NX_SET_SIZE - 2), "Invalid NX_STATE_MAX");
struct nx {
    size_t n_states;
    struct nx_state states[NX_STATE_MAX];

    char * expression;
    struct nx_set head_states;
};

NOODLE_EXPORT struct nx * nx_compile(const char * expression);
NOODLE_EXPORT void nx_destroy(struct nx * nx);
NOODLE_EXPORT void nx_test(void);
NOODLE_EXPORT int nx_match(const struct nx * nx, const char * input, size_t n_errors);

struct nx_set nx_match_partial(const struct nx * nx, const enum nx_char * buffer, uint16_t si);
