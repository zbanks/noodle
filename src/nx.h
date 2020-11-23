#pragma once

#include "prelude.h"

//
// `struct nx_set` is a bitset representation with a hardcoded maximum size `NX_SET_SIZE`
//

#define NX_SET_SIZE ((size_t)256)
// For compatability with python/cffi, we are limited in what math we can do in constant decls
#define NX_SET_ARRAYLEN ((size_t)4)
//#define NX_SET_ARRAYLEN ((NX_SET_SIZE + 63) / 64)
_Static_assert(NX_SET_ARRAYLEN == ((NX_SET_SIZE + 63) / 64), "Invalid NX_SET_ARRAYLEN");

struct nx_set {
    uint64_t xs[NX_SET_ARRAYLEN];
};

// Return true if the set is entirely empty (0)
bool nx_set_isempty(const struct nx_set * s);
// Return true if the bit `i` is a valid bit and is set
bool nx_set_test(const struct nx_set * s, size_t i);
// Set bit `i` and return true if `i` is a valid bit and was not previously set
bool nx_set_add(struct nx_set * s, size_t i);

//
// `enum nx_char` is a 5-bit representation of the allowed letters, plus some metacharacters
//
// Although this encoding does not save space when storing *strings*, it yields smaller lookup
// tables & state representations than 8-bit ASCII.
//

enum nx_char {
    // End of string, like '\0' on normal C strings
    NX_CHAR_END = 0,
    // "Epsilon" state transition, from regex/NFA literature
    NX_CHAR_EPSILON,
    // Catch-all character for otherwise untranslatable characters from source strings
    NX_CHAR_INVALID,
    // Whitespace character
    NX_CHAR_SPACE,

    // A...Z, case-insensitive
    NX_CHAR_A,
    NX_CHAR_Z = NX_CHAR_A + 25,

    _NX_CHAR_MAX,
};

// Convert an `input` C string into an `output` sequence of `enum nx_char`s
void nx_char_translate(const char * input, enum nx_char * output, size_t output_size);

//
// `struct nx_state` is the representation of a single node in a NFA, or
// Non-deterministic Finite Automata.
//

#define NX_BRANCH_COUNT ((size_t)2)
struct nx_state {
    enum {
        STATE_TYPE_TRANSITION,
        // STATE_ANAGRAM_EXACT,
        // STATE_ANAGRAM_LIMIT,
    } type;
    union {
        struct {
            // A fully-general NFA node could have an arbitrary number of outgoing
            // edges, but WLOG we only allow `NX_BRANCH_COUNT` (2).
            // The outgoing edges are defined with `next_state` &  `char_bitset`.
            //
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
            // evaluated in ~linear time*.
            //
            // (It's only linear time if we assume the number of states is O(1),
            // which is not a traditional assumption to make!)
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

//
// `struct nx` is a representation of a compiled NX expression (primarily the NFA).
//

// The number of allowed states in an NX NFA is limited by the bitset representation,
// leaving two states for success & failure.
#define NX_STATE_MAX ((size_t)254)
//#define NX_STATE_MAX (NX_SET_SIZE - 2)
_Static_assert(NX_STATE_MAX == (NX_SET_SIZE - 2), "Invalid NX_STATE_MAX");

struct nx {
    // The NFA state table
    size_t n_states;
    struct nx_state states[NX_STATE_MAX];

    // The original NX expression, as text, for debugging
    char * expression;

    // Which states are not immediately reachable from another state via
    // an epsilon transition? This is used as a heuristic for performing
    // combo searches (and may include some states that *are* reachable)
    struct nx_set head_states;
};

// Compile an NX text expression into a `struct nx` object
NOODLE_EXPORT struct nx * nx_compile(const char * expression);
// Destroy/free a `struct nx` object
NOODLE_EXPORT void nx_destroy(struct nx * nx);
// Check if `input` matches `nx`, with a tolerance of up to `n_errors`.
// Returns `-1` if not, otherwise returns the Levenshtein distance from the NX expression:
// always <= `n_errors`, and `0` for a perfect match.
NOODLE_EXPORT int nx_match(const struct nx * nx, const char * input, size_t n_errors);

// Run some internal tests
NOODLE_EXPORT void nx_test(void);

// Incremental match used by `nx_combo_match`.
// From a given `initial_state`, return the set of possible result states
// after consuming every character in `buffer`.
struct nx_set nx_match_partial(const struct nx * nx, const enum nx_char * buffer, uint16_t initial_state);
