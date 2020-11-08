#include "nx.h"
#include <time.h>

#if 1
#define NX_SET_SIZE ((size_t)64)
struct nx_set {
    uint64_t xs[NX_SET_SIZE / 64];
};

static const struct nx_set NX_SET_START = {{1}};

static bool nx_set_test(const struct nx_set * s, size_t i) {
    if (i >= NX_SET_SIZE) {
        return false;
    }
    return (s->xs[i / 64] & (1ul << (i % 64))) != 0;
}

static bool nx_set_isempty(const struct nx_set * s) {
    for (size_t i = 0; i < NX_SET_SIZE / 64; i++) {
        if (s->xs[i]) {
            return false;
        }
    }
    return true;
}

static bool nx_set_add(struct nx_set * s, size_t i) {
    if (i >= NX_SET_SIZE) {
        return false;
    }
    if (nx_set_test(s, i)) {
        return false;
    }
    s->xs[i / 64] |= (1ul << (i % 64));
    return true;
}

static void nx_set_orequal(struct nx_set * restrict s, const struct nx_set * restrict t) {
    for (size_t i = 0; i < NX_SET_SIZE / 64; i++) {
        s->xs[i] |= t->xs[i];
    }
}
#else
#define NX_SET_SIZE ((size_t)64)
struct nx_set {
    uint64_t x;
};

static const struct nx_set NX_SET_START = {1};

static inline bool nx_set_test(const struct nx_set * s, size_t i) {
    if (i >= NX_SET_SIZE) {
        return false;
    }
    return (s->x & (1ul << i)) != 0;
}

static inline bool nx_set_isempty(const struct nx_set * s) { return s->x == 0; }

static inline bool nx_set_add(struct nx_set * s, size_t i) {
    if (i >= NX_SET_SIZE) {
        return false;
    }
    if (nx_set_test(s, i)) {
        return false;
    }
    s->x |= (1ul << i);
    return true;
}

static inline void nx_set_orequal(struct nx_set * restrict s, const struct nx_set * restrict t) { s->x |= t->x; }
#endif

const char * nx_set_debug(const struct nx_set * s) {
    static char buffer[NX_SET_SIZE * 6];
    char * b = buffer;
    bool first = true;
    for (size_t i = 0; i < NX_SET_SIZE; i++) {
        if (nx_set_test(s, i)) {
            if (!first) {
                b += sprintf(b, ",");
            }
            b += sprintf(b, "%zu", i);
            first = false;
        }
    }
    if (first) {
        b += sprintf(b, "(empty)");
    }
    return buffer;
}

enum nx_char {
    NX_CHAR_END = 0,
    NX_CHAR_EPSILON,
    NX_CHAR_INVALID,
    NX_CHAR_SPACE,

    NX_CHAR_A,
    NX_CHAR_Z = NX_CHAR_A + 25,

    _NX_CHAR_MAX,
};

_Static_assert(_NX_CHAR_MAX <= 31, "Unexpectedly large enum nx_char");

enum {
    STATE_SUCCESS = NX_SET_SIZE - 1,
    STATE_FAILURE = NX_SET_SIZE,
};

#define NX_STATE_MAX (NX_SET_SIZE - 2)
_Static_assert(NX_STATE_MAX < UINT16_MAX, "NX_STATE_MAX too big for a uint16_t");

#define NX_BRANCH_COUNT ((size_t)2)
_Static_assert(_NX_CHAR_MAX < 32, "_NX_CHAR_MAX cannot fit in a uint32_t");

struct nx_state {
    enum {
        STATE_TYPE_TRANSITION,
        STATE_ANAGRAM_EXACT,
        STATE_ANAGRAM_LIMIT,
    } type;
    union {
        struct {
            uint16_t next_state[NX_BRANCH_COUNT];
            uint32_t char_bitset[NX_BRANCH_COUNT];
            struct nx_set epsilon_states;
        };
        /*
        struct {
            uint16_t transition_fail;
            uint16_t transition_success;
            int16_t anagram_arg;
            uint8_t anagram_letters[(_NX_CHAR_MAX - 4) * 2];
        };
        */
    };
};

struct nx {
    char * expression;

    size_t n_states;
    struct nx_state states[NX_STATE_MAX];
};

static enum nx_char nx_char(char c) {
    switch (c) {
    case '\0':
        return NX_CHAR_END;
    case ' ':
        return NX_CHAR_SPACE;
    case 'A' ... 'Z':
        return NX_CHAR_A + (c - 'A');
    case 'a' ... 'z':
        return NX_CHAR_A + (c - 'a');
    default:
        return NX_CHAR_INVALID;
    }
}

static char nx_char_rev_print(enum nx_char c) {
    switch (c) {
    case NX_CHAR_END:
        return '$';
    case NX_CHAR_EPSILON:
        return '*';
    case NX_CHAR_SPACE:
        return '_';
    case NX_CHAR_A... NX_CHAR_Z:
        return (char)('a' + (c - NX_CHAR_A));
    default:
        LOG("Unknown char: %d", c);
        return '?';
    }
}

static uint32_t nx_char_bit(enum nx_char c) { return (1u << ((uint8_t)c)); }

static const char * nx_char_set_debug(uint32_t cs) {
    static char buffer[1024];
    char * b = buffer;
    b += sprintf(b, "[");
    for (enum nx_char c = 0; c <= _NX_CHAR_MAX; c++) {
        if (cs & nx_char_bit(c)) {
            b += sprintf(b, "%c", nx_char_rev_print(c));
        }
    }
    b += sprintf(b, "]");
    return buffer;
}

static void nx_nfa_debug(const struct nx * nx) {
    LOG("NX NFA: %zu states", nx->n_states);
    for (size_t i = 0; i < nx->n_states; i++) {
        const struct nx_state * s = &nx->states[i];
        ASSERT(s->type == STATE_TYPE_TRANSITION);

        printf("     %3zu: ", i);
        for (size_t j = 0; j < NX_BRANCH_COUNT; j++) {
            if (s->char_bitset[j] == 0) {
                // These two cases are just to catch potentially-invalid representations
                if (j + 1 < NX_BRANCH_COUNT && s->char_bitset[j + 1] != 0) {
                    printf("(missing %zu)    ", j);
                }
                // 0 is technically a valid state; this just catches _most_ errors
                if (s->next_state[j] != 0) {
                    printf("(null) -> %hu    ", s->next_state[j]);
                }
                continue;
            }
            printf("%s -> ", nx_char_set_debug(s->char_bitset[j]));
            if (s->next_state[j] > STATE_SUCCESS) {
                printf("!!!%hu", s->next_state[j]);
            } else if (s->next_state[j] == STATE_SUCCESS) {
                printf("MATCH");
            } else {
                printf("%-3hu", s->next_state[j]);
            }
            printf("      ");
        }
        printf("epsilon: %s", nx_set_debug(&s->epsilon_states));
        printf("\n");
    }
    printf("\n");
}

struct nx_state * nx_state_insert(struct nx * nx, size_t insert_index) {
    ASSERT(insert_index < (NX_STATE_MAX - 1));
    ASSERT(insert_index < nx->n_states);
    size_t remaining_states = nx->n_states - insert_index;
    memmove(&nx->states[insert_index + 1], &nx->states[insert_index], remaining_states * sizeof(*nx->states));
    memset(&nx->states[insert_index], 0, sizeof(*nx->states));

    nx->n_states++;
    ASSERT(nx->n_states <= NX_STATE_MAX);

    for (size_t i = insert_index + 1; i < nx->n_states; i++) {
        ASSERT(nx->states[i].type == STATE_TYPE_TRANSITION);
        for (size_t j = 0; j < NX_BRANCH_COUNT; j++) {
            if (nx->states[i].next_state[j] >= insert_index && nx->states[i].next_state[j] < nx->n_states &&
                nx->states[i].char_bitset[j] != 0) {
                nx->states[i].next_state[j]++;
            }
        }
    }
    return &nx->states[insert_index];
}

ssize_t nx_compile_subexpression(struct nx * nx, const char * subexpression) {
    ssize_t consumed_characters = 0;
    size_t previous_initial_state = STATE_FAILURE;
    size_t subexpression_initial_state = nx->n_states;
    size_t subexpression_final_state = STATE_FAILURE;
    for (const char * c = subexpression;; c++) {
        struct nx_state * s = &nx->states[nx->n_states];
        ASSERT(nx->n_states < NX_STATE_MAX);

        enum nx_char nc = nx_char(*c);
        switch (*c) {
        case ')':
            if (subexpression_final_state != STATE_FAILURE) {
                LOG("Subexpression %zu", subexpression_final_state);
                nx->states[subexpression_final_state].next_state[0] = (uint16_t)(nx->n_states);
            }
            return consumed_characters;
        case '\0':
            s->type = STATE_TYPE_TRANSITION;
            s->next_state[0] = STATE_SUCCESS;
            s->char_bitset[0] = nx_char_bit(NX_CHAR_END);

            nx->n_states++;
            if (subexpression_final_state != STATE_FAILURE) {
                LOG("Subexpression %zu", subexpression_final_state);
                nx->states[subexpression_final_state].next_state[0] = (uint16_t)(nx->n_states);
            }
            return consumed_characters;
        case 'A' ... 'Z':
        case 'a' ... 'z':
            s->type = STATE_TYPE_TRANSITION;
            s->next_state[0] = (uint16_t)(nx->n_states + 1);
            s->char_bitset[0] = nx_char_bit(nc);
            s->next_state[1] = (uint16_t)(nx->n_states);
            s->char_bitset[1] = nx_char_bit(NX_CHAR_SPACE);

            previous_initial_state = nx->n_states;
            nx->n_states++;
            break;
        case ' ':
            break;
        case '_': // Explicit space
            s->type = STATE_TYPE_TRANSITION;
            s->next_state[0] = (uint16_t)(nx->n_states + 1);
            s->char_bitset[0] = nx_char_bit(NX_CHAR_SPACE);

            previous_initial_state = nx->n_states;
            nx->n_states++;
            break;
        case '[':
            c++;
            consumed_characters++;
            bool inverse = false;
            if (*c == '^') {
                inverse = true;
                c++;
                consumed_characters++;
            }

            s->type = STATE_TYPE_TRANSITION;
            s->next_state[0] = (uint16_t)(nx->n_states + 1);
            s->char_bitset[0] = 0;
            s->next_state[1] = (uint16_t)(nx->n_states);
            s->char_bitset[1] = nx_char_bit(NX_CHAR_SPACE);

            while (*c != ']' && *c != '\0') {
                if (nx_char(*c) >= NX_CHAR_A && nx_char(*c) <= NX_CHAR_Z) {
                    s->char_bitset[0] |= nx_char_bit(nx_char(*c));
                } else {
                    LOG("Parse error; invalid character '%c' in [...] group", *c);
                    return -1;
                }
                c++;
                consumed_characters++;
            }
            if (*c == '\0') {
                LOG("Parse error; unterminated [");
                return -1;
            }
            if (inverse) {
                for (enum nx_char j = NX_CHAR_A; j <= NX_CHAR_Z; j++) {
                    s->char_bitset[0] ^= nx_char_bit(j);
                }
            }
            previous_initial_state = nx->n_states;
            nx->n_states++;
            break;
        case '*': {
            s->type = STATE_TYPE_TRANSITION;
            if (previous_initial_state == STATE_FAILURE) {
                LOG("nx parse error: '%c' without preceeding group", *c);
                return -1;
            }

            struct nx_state * epsilon_s = nx_state_insert(nx, previous_initial_state++);
            if (previous_initial_state < subexpression_final_state && subexpression_final_state != STATE_FAILURE) {
                subexpression_final_state++;
            }
            epsilon_s->type = STATE_TYPE_TRANSITION;
            epsilon_s->next_state[0] = (uint16_t)previous_initial_state;
            epsilon_s->char_bitset[0] = nx_char_bit(NX_CHAR_EPSILON);
            epsilon_s->next_state[1] = (uint16_t)(nx->n_states + 1);
            epsilon_s->char_bitset[1] = nx_char_bit(NX_CHAR_EPSILON);

            s = &nx->states[nx->n_states];
            s->type = STATE_TYPE_TRANSITION;
            s->next_state[0] = (uint16_t)previous_initial_state;
            s->char_bitset[0] = nx_char_bit(NX_CHAR_EPSILON);
            s->next_state[1] = (uint16_t)(nx->n_states + 1);
            s->char_bitset[1] = nx_char_bit(NX_CHAR_EPSILON);

            nx->n_states++;
            break;
        }
        case '+':
            s->type = STATE_TYPE_TRANSITION;
            if (previous_initial_state == STATE_FAILURE) {
                LOG("nx parse error: '%c' without preceeding group", *c);
                return -1;
            }

            s = &nx->states[nx->n_states];
            s->type = STATE_TYPE_TRANSITION;
            s->next_state[0] = (uint16_t)previous_initial_state;
            s->char_bitset[0] = nx_char_bit(NX_CHAR_EPSILON);
            s->next_state[1] = (uint16_t)(nx->n_states + 1);
            s->char_bitset[1] = nx_char_bit(NX_CHAR_EPSILON);

            nx->n_states++;
            break;
        case '?': {
            s->type = STATE_TYPE_TRANSITION;
            if (previous_initial_state == STATE_FAILURE) {
                LOG("nx parse error: '%c' without preceeding group", *c);
                return -1;
            }

            struct nx_state * epsilon_s = nx_state_insert(nx, previous_initial_state++);
            if (previous_initial_state < subexpression_final_state && subexpression_final_state != STATE_FAILURE) {
                subexpression_final_state++;
            }
            epsilon_s->type = STATE_TYPE_TRANSITION;
            epsilon_s->next_state[0] = (uint16_t)previous_initial_state;
            epsilon_s->char_bitset[0] = nx_char_bit(NX_CHAR_EPSILON);
            epsilon_s->next_state[1] = (uint16_t)(nx->n_states);
            epsilon_s->char_bitset[1] = nx_char_bit(NX_CHAR_EPSILON);

            break;
        }
        case '(':
            c++;
            consumed_characters++;

            previous_initial_state = nx->n_states;
            ssize_t rc = nx_compile_subexpression(nx, c);
            if (rc < 0 || c[rc] != ')') {
                LOG("nx parse error: invalid (...) group");
                return -1;
            }

            c += rc;
            consumed_characters += rc;
            break;
        case '|': {
            struct nx_state * epsilon_s = nx_state_insert(nx, subexpression_initial_state);
            if (subexpression_final_state != STATE_FAILURE) {
                subexpression_final_state++;
            }

            epsilon_s->type = STATE_TYPE_TRANSITION;
            epsilon_s->next_state[0] = (uint16_t)(subexpression_initial_state + 1);
            epsilon_s->char_bitset[0] = nx_char_bit(NX_CHAR_EPSILON);
            epsilon_s->next_state[1] = (uint16_t)(nx->n_states);
            epsilon_s->char_bitset[1] = nx_char_bit(NX_CHAR_EPSILON);

            if (subexpression_final_state == STATE_FAILURE) {
                subexpression_final_state = nx->n_states;
                s = &nx->states[subexpression_final_state];
                s->type = STATE_TYPE_TRANSITION;
                s->next_state[0] = STATE_FAILURE; // This is filled in at the end
                s->char_bitset[0] = nx_char_bit(NX_CHAR_EPSILON);
                epsilon_s->next_state[1]++;
                nx->n_states++;
            } else {
                ASSERT(nx->n_states > 0);
                struct nx_state * last_s = &nx->states[nx->n_states - 1];
                for (size_t j = 0; j < NX_BRANCH_COUNT; j++) {
                    if (last_s->next_state[j] == (uint16_t)nx->n_states) {
                        last_s->next_state[j] = (uint16_t)subexpression_final_state;
                    }
                }
            }

            previous_initial_state = STATE_FAILURE;
            break;
        }
        default:
            break;
        }

        consumed_characters++;
    }
}

struct nx * nx_compile(const char * expression) {
    NONNULL(expression);

    struct nx * nx = NONNULL(calloc(1, sizeof(*nx)));
    nx->expression = NONNULL(strdup(expression));

    if (0)
        goto fail;

    ssize_t rc = nx_compile_subexpression(nx, nx->expression);
    if (rc < 0)
        goto fail;
    ASSERT(rc == (ssize_t)strlen(nx->expression));

    // Calculate epsilon transitions
    for (size_t i = 0; i < nx->n_states; i++) {
        struct nx_state * s = &nx->states[i];
        struct nx_set next_ss;
        memset(&next_ss, 0, sizeof(next_ss));
        for (size_t j = 0; j < NX_BRANCH_COUNT; j++) {
            if (nx_char_bit(NX_CHAR_EPSILON) & s->char_bitset[j]) {
                nx_set_add(&next_ss, s->next_state[j]);
            }
        }
        for (size_t k = 0; k < 10; k++) {
            struct nx_set ss = next_ss;
            for (size_t si = 0; si < nx->n_states; si++) {
                if (!nx_set_test(&next_ss, si)) {
                    continue;
                }
                const struct nx_state * s2 = &nx->states[si];
                ASSERT(s2->type == STATE_TYPE_TRANSITION);

                for (size_t j = 0; j < NX_BRANCH_COUNT; j++) {
                    if (nx_char_bit(NX_CHAR_EPSILON) & s2->char_bitset[j]) {
                        nx_set_add(&ss, s2->next_state[j]);
                    }
                }
            }
            nx_set_orequal(&next_ss, &ss);
            if (memcmp(&ss, &next_ss, sizeof(ss)) == 0) {
                break;
            }
        }
        s->epsilon_states = next_ss;
    }

    LOG("Created NFA for \"%s\" with %zu states", expression, nx->n_states);
    nx_nfa_debug(nx);

    return nx;

fail:
    nx_destroy(nx);
    return NULL;
}

void nx_destroy(struct nx * nx) {
    if (nx == NULL) {
        return;
    }
    free(nx->expression);
    free(nx);
}

static struct nx_set nx_match_transition(const struct nx * nx, enum nx_char b, struct nx_set ss) {
    struct nx_set new_ss = {0};
    if (nx_set_isempty(&ss)) {
        return new_ss;
    }
    for (size_t si = 0; si < NX_STATE_MAX; si++) {
        if (!nx_set_test(&ss, si)) {
            continue;
        }
        const struct nx_state * s = &nx->states[si];
        ASSERT(s->type == STATE_TYPE_TRANSITION);
        nx_set_orequal(&new_ss, &s->epsilon_states);
        for (size_t j = 0; j < NX_BRANCH_COUNT; j++) {
            if (nx_char_bit(b) & s->char_bitset[j]) {
                nx_set_add(&new_ss, s->next_state[j]);
            }
        }
    }
    return new_ss;
}

static int nx_match_fuzzy(const struct nx * nx, const enum nx_char * buffer, size_t bi, struct nx_set ss,
                          size_t n_errors) {
    if (nx_set_test(&ss, STATE_SUCCESS)) {
        return 0;
    }
    struct nx_set err_ss = {0};
    while (true) {
        struct nx_set next_ss = nx_match_transition(nx, buffer[bi], ss);
        struct nx_set next_err_ss = nx_match_transition(nx, buffer[bi], err_ss);
        size_t next_bi = bi + 1;
        if (nx_set_test(&next_ss, STATE_SUCCESS)) {
            return 0;
        }
        if (nx_set_test(&next_err_ss, STATE_SUCCESS)) {
            return 1;
        }
        if (n_errors > 0) {
            if (buffer[bi] != NX_CHAR_END) {
                // Try deleting a char
                {
                    struct nx_set es = nx_match_transition(nx, buffer[next_bi], ss);
                    nx_set_orequal(&next_err_ss, &es);
                }

                // Try changing the char
                for (enum nx_char alt = NX_CHAR_END + 1; alt <= NX_CHAR_Z; alt++) {
                    struct nx_set es = nx_match_transition(nx, alt, ss);
                    nx_set_orequal(&next_err_ss, &es);
                }
            }

            // Try inserting a char
            for (enum nx_char alt = NX_CHAR_END + 1; alt <= NX_CHAR_Z; alt++) {
                struct nx_set es = nx_match_transition(nx, alt, next_ss);
                nx_set_orequal(&next_err_ss, &es);
            }
        }

        if (nx_set_isempty(&next_ss) || buffer[bi] == NX_CHAR_END) {
            if (n_errors > 0) {
                int rc = nx_match_fuzzy(nx, buffer, next_bi, next_err_ss, n_errors - 1);
                if (rc >= 0) {
                    return rc + 1;
                }
            }
            // LOG("No matches after state %s", nx_set_debug(&ss));
            return -1;
        }

        ASSERT(buffer[bi] != NX_CHAR_END);

        ss = next_ss;
        err_ss = next_err_ss;
        bi = next_bi;
    }
}

int nx_match(const struct nx * nx, const char * input, size_t n_errors) {
    enum nx_char buffer[256];
    for (size_t i = 0;; i++) {
        buffer[i] = nx_char(input[i]);
        if (buffer[i] == NX_CHAR_END) {
            break;
        }
    }

    struct nx_set ss = nx_match_transition(nx, NX_CHAR_EPSILON, NX_SET_START);
    nx_set_orequal(&ss, &NX_SET_START);
    return nx_match_fuzzy(nx, buffer, 0, ss, n_errors);
}

static int64_t now() {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec * 1000000000 + t.tv_nsec;
}

void nx_test(void) {
    // struct nx * nx = nx_compile("([^asdfzyxwv]el([lw]o)+r[lheld]*)+");
    // struct nx * nx = nx_compile("he?a?z?l+?oworld");
    // struct nx * nx = nx_compile("(thing|hello|asdf|world)*");
    struct nx * nx = nx_compile("helloworld");
    const char * s[] = {
        "helloworld",  "hello",     "helloworldhello", "helloworldhelloworld", "h e l l o w o r l d",  "helloworl",
        "helloworlda", "heloworld", "hellloworld",     "hellaworld",           "aaaaasdfawjeojworkld", "heoworld",
        NULL,
    };
    for (size_t i = 0; s[i] != NULL; i++) {
        int64_t t = now();
        int rc = nx_match(nx, s[i], 0);
        t = now() - t;
        LOG("> \"%s\": %d in %ld ns", s[i], rc, t);
    }
}
