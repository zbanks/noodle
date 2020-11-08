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
    return buffer;
}

enum nx_char {
    NX_CHAR_END = 0,
    NX_CHAR_EPSILON1,
    NX_CHAR_EPSILON2,
    NX_CHAR_INVALID,
    NX_CHAR_SPACE,

    NX_CHAR_A,
    NX_CHAR_Z = NX_CHAR_A + 25,

    _NX_CHAR_MAX,
};

_Static_assert(_NX_CHAR_MAX <= 31, "Unexpectedly large enum nx_char");

enum nx_transition {
    TRANSITION_SUCCESS = NX_SET_SIZE - 1,
    TRANSITION_FAIL = NX_SET_SIZE,
};

#define NX_STATE_MAX (NX_SET_SIZE - 2)
_Static_assert(NX_STATE_MAX < UINT16_MAX, "NX_STATE_MAX too big for a uint16_t");

struct nx_state {
    enum {
        STATE_TRANSITION,
        STATE_ANAGRAM_EXACT,
        STATE_ANAGRAM_LIMIT,
    } type;
    union {
        uint16_t transition_table[_NX_CHAR_MAX];
        struct {
            uint16_t transition_fail;
            uint16_t transition_success;
            int16_t anagram_arg;
            uint8_t anagram_letters[(_NX_CHAR_MAX - 4) * 2];
        };
    };
};

struct nx {
    char * expression;

    size_t n_states;
    struct nx_state states[NX_STATE_MAX];
};

enum nx_char nx_char(char c) {
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

char nx_char_rev(enum nx_char c) {
    switch (c) {
    case NX_CHAR_END:
        return '\0';
    case NX_CHAR_SPACE:
        return ' ';
    case NX_CHAR_A... NX_CHAR_Z:
        return (char)('a' + (c - NX_CHAR_A));
    default:
        return '?';
    }
}

char nx_char_rev_print(enum nx_char c) {
    switch (c) {
    case NX_CHAR_END:
        return '$';
    case NX_CHAR_EPSILON1:
        return '1';
    case NX_CHAR_EPSILON2:
        return '2';
    case NX_CHAR_SPACE:
        return '_';
    case NX_CHAR_A... NX_CHAR_Z:
        return (char)('a' + (c - NX_CHAR_A));
    default:
        return '?';
    }
}

struct nx_state * nx_state_insert(struct nx * nx, size_t insert_index) {
    ASSERT(insert_index < (NX_STATE_MAX - 1));
    ASSERT(insert_index < nx->n_states);
    size_t remaining_states = nx->n_states - insert_index;
    memmove(&nx->states[insert_index + 1], &nx->states[insert_index], remaining_states * sizeof(*nx->states));

    nx->n_states++;
    ASSERT(nx->n_states <= NX_STATE_MAX);

    for (size_t i = insert_index + 1; i < nx->n_states; i++) {
        ASSERT(nx->states[i].type == STATE_TRANSITION);
        for (enum nx_char j = 0; j < _NX_CHAR_MAX; j++) {
            if (nx->states[i].transition_table[j] >= insert_index && nx->states[i].transition_table[j] < nx->n_states) {
                nx->states[i].transition_table[j]++;
            }
        }
    }
    return &nx->states[insert_index];
}

ssize_t nx_compile_subexpression(struct nx * nx, const char * subexpression) {
    ssize_t consumed_characters = 0;
    size_t previous_initial_state = TRANSITION_FAIL;
    size_t subexpression_initial_state = nx->n_states;
    size_t subexpression_final_state = TRANSITION_FAIL;
    for (const char * c = subexpression;; c++) {
        struct nx_state * s = &nx->states[nx->n_states];
        ASSERT(nx->n_states < NX_STATE_MAX);

        enum nx_char nc = nx_char(*c);
        switch (*c) {
        case ')':
            if (subexpression_final_state != TRANSITION_FAIL) {
                LOG("Subexpression %zu", subexpression_final_state);
                nx->states[subexpression_final_state].transition_table[NX_CHAR_EPSILON1] = (uint16_t)(nx->n_states);
            }
            return consumed_characters;
        case '\0':
            s->type = STATE_TRANSITION;
            for (enum nx_char i = 0; i < _NX_CHAR_MAX; i++) {
                if (i == NX_CHAR_END) {
                    s->transition_table[i] = TRANSITION_SUCCESS;
                } else {
                    s->transition_table[i] = TRANSITION_FAIL;
                }
            }
            nx->n_states++;
            if (subexpression_final_state != TRANSITION_FAIL) {
                LOG("Subexpression %zu", subexpression_final_state);
                nx->states[subexpression_final_state].transition_table[NX_CHAR_EPSILON1] = TRANSITION_SUCCESS;
            }
            return consumed_characters;
        case 'A' ... 'Z':
        case 'a' ... 'z':
            s->type = STATE_TRANSITION;
            for (enum nx_char i = 0; i < _NX_CHAR_MAX; i++) {
                if (i == nc) {
                    s->transition_table[i] = (uint16_t)(nx->n_states + 1);
                } else if (i == NX_CHAR_SPACE) {
                    s->transition_table[i] = (uint16_t)nx->n_states;
                } else {
                    s->transition_table[i] = TRANSITION_FAIL;
                }
            }
            previous_initial_state = nx->n_states;
            nx->n_states++;
            break;
        case ' ':
            break;
        case '_': // Explicit space
            s->type = STATE_TRANSITION;
            for (enum nx_char i = 0; i < _NX_CHAR_MAX; i++) {
                if (i == NX_CHAR_SPACE) {
                    s->transition_table[i] = (uint16_t)(nx->n_states + 1);
                } else {
                    s->transition_table[i] = TRANSITION_FAIL;
                }
            }
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

            s->type = STATE_TRANSITION;
            for (enum nx_char i = 0; i < _NX_CHAR_MAX; i++) {
                if (i == NX_CHAR_SPACE) {
                    s->transition_table[i] = (uint16_t)nx->n_states;
                } else if (i >= NX_CHAR_A && i <= NX_CHAR_Z) {
                    s->transition_table[i] = inverse ? (uint16_t)(nx->n_states + 1) : TRANSITION_FAIL;
                } else {
                    s->transition_table[i] = TRANSITION_FAIL;
                }
            }
            while (*c != ']' && *c != '\0') {
                for (enum nx_char i = 0; i < _NX_CHAR_MAX; i++) {
                    if (i == nx_char(*c)) {
                        s->transition_table[i] = inverse ? TRANSITION_FAIL : (uint16_t)(nx->n_states + 1);
                    }
                }
                c++;
                consumed_characters++;
            }
            if (*c == '\0') {
                LOG("Parse error; unterminated [");
                return -1;
            }
            previous_initial_state = nx->n_states;
            nx->n_states++;
            break;
        case '*': {
            s->type = STATE_TRANSITION;
            if (previous_initial_state == TRANSITION_FAIL) {
                LOG("nx parse error: '%c' without preceeding group", *c);
                return -1;
            }

            struct nx_state * epsilon_s = nx_state_insert(nx, previous_initial_state++);
            if (previous_initial_state < subexpression_final_state && subexpression_final_state != TRANSITION_FAIL) {
                subexpression_final_state++;
            }
            epsilon_s->type = STATE_TRANSITION;

            s = &nx->states[nx->n_states];
            s->type = STATE_TRANSITION;

            for (enum nx_char i = 0; i < _NX_CHAR_MAX; i++) {
                if (i == NX_CHAR_EPSILON1) {
                    epsilon_s->transition_table[i] = (uint16_t)(previous_initial_state);
                    s->transition_table[i] = (uint16_t)(previous_initial_state);
                } else if (i == NX_CHAR_EPSILON2) {
                    epsilon_s->transition_table[i] = (uint16_t)(nx->n_states + 1);
                    s->transition_table[i] = (uint16_t)(nx->n_states + 1);
                } else {
                    epsilon_s->transition_table[i] = TRANSITION_FAIL;
                    s->transition_table[i] = TRANSITION_FAIL;
                }
            }

            // previous_initial_state = nx->n_states;
            nx->n_states++;
            break;
        }
        case '+':
            s->type = STATE_TRANSITION;
            if (previous_initial_state == TRANSITION_FAIL) {
                LOG("nx parse error: '%c' without preceeding group", *c);
                return -1;
            }

            s = &nx->states[nx->n_states];
            s->type = STATE_TRANSITION;

            for (enum nx_char i = 0; i < _NX_CHAR_MAX; i++) {
                if (i == NX_CHAR_EPSILON1) {
                    s->transition_table[i] = (uint16_t)(previous_initial_state);
                } else if (i == NX_CHAR_EPSILON2) {
                    s->transition_table[i] = (uint16_t)(nx->n_states + 1);
                } else {
                    s->transition_table[i] = TRANSITION_FAIL;
                }
            }

            // previous_initial_state = nx->n_states;
            nx->n_states++;
            break;
        case '?': {
            s->type = STATE_TRANSITION;
            if (previous_initial_state == TRANSITION_FAIL) {
                LOG("nx parse error: '%c' without preceeding group", *c);
                return -1;
            }

            struct nx_state * epsilon_s = nx_state_insert(nx, previous_initial_state++);
            if (previous_initial_state < subexpression_final_state && subexpression_final_state != TRANSITION_FAIL) {
                subexpression_final_state++;
            }
            epsilon_s->type = STATE_TRANSITION;
            for (enum nx_char i = 0; i < _NX_CHAR_MAX; i++) {
                if (i == NX_CHAR_EPSILON1) {
                    epsilon_s->transition_table[i] = (uint16_t)(previous_initial_state);
                } else if (i == NX_CHAR_EPSILON2) {
                    epsilon_s->transition_table[i] = (uint16_t)(nx->n_states);
                } else {
                    epsilon_s->transition_table[i] = TRANSITION_FAIL;
                }
            }

            // previous_initial_state = nx->n_states;
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
            if (subexpression_final_state != TRANSITION_FAIL) {
                subexpression_final_state++;
            }

            epsilon_s->type = STATE_TRANSITION;
            for (enum nx_char i = 0; i < _NX_CHAR_MAX; i++) {
                if (i == NX_CHAR_EPSILON1) {
                    epsilon_s->transition_table[i] = (uint16_t)(subexpression_initial_state + 1);
                } else if (i == NX_CHAR_EPSILON2) {
                    epsilon_s->transition_table[i] = (uint16_t)(nx->n_states);
                } else {
                    epsilon_s->transition_table[i] = TRANSITION_FAIL;
                }
            }

            if (subexpression_final_state == TRANSITION_FAIL) {
                subexpression_final_state = nx->n_states;
                s = &nx->states[nx->n_states];
                s->type = STATE_TRANSITION;
                for (enum nx_char i = 0; i < _NX_CHAR_MAX; i++) {
                    s->transition_table[i] = TRANSITION_FAIL;
                }
                epsilon_s->transition_table[NX_CHAR_EPSILON2]++;
                nx->n_states++;
            } else {
                ASSERT(nx->n_states > 0);
                struct nx_state * last_s = &nx->states[nx->n_states - 1];
                for (enum nx_char i = 0; i < _NX_CHAR_MAX; i++) {
                    if (last_s->transition_table[i] == (uint16_t)nx->n_states) {
                        last_s->transition_table[i] = (uint16_t)subexpression_final_state;
                    }
                }
            }

            previous_initial_state = TRANSITION_FAIL;
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

    LOG("Created NFA for \"%s\" with %zu states", expression, nx->n_states);

    return nx;

fail:
    nx_destroy(nx);
    return NULL;
}

void nx_print_nfa(const struct nx * nx) {
    LOG("NX NFA: %zu states", nx->n_states);
    for (size_t i = 0; i < nx->n_states; i++) {
        const struct nx_state * s = &nx->states[i];
        ASSERT(s->type == STATE_TRANSITION);

        printf("     [%3zu]: ", i);
        for (enum nx_char j = 0; j < _NX_CHAR_MAX; j++) {
            if (s->transition_table[j] == TRANSITION_FAIL) {
                continue;
            } else if (s->transition_table[j] == TRANSITION_SUCCESS) {
                printf("%c->MATCH  ", nx_char_rev_print(j));
            } else if (j != NX_CHAR_SPACE) {
                printf("%c->%hu  ", nx_char_rev_print(j), s->transition_table[j]);
            }
        }
        printf("\n");
    }
    printf("\n");
}

void nx_destroy(struct nx * nx) {
    if (nx == NULL) {
        return;
    }
    free(nx->expression);
    free(nx);
}

static struct nx_set nx_match_transition2(const struct nx * nx, enum nx_char b, const struct nx_set ss) {
    struct nx_set new_ss = {0};
    for (size_t si = 0; si < NX_STATE_MAX; si++) {
        if (!nx_set_test(&ss, si)) {
            continue;
        }
        const struct nx_state * s = &nx->states[si];
        ASSERT(s->type == STATE_TRANSITION);

        if (b == NX_CHAR_EPSILON1 || b == NX_CHAR_EPSILON2) {
            nx_set_add(&new_ss, s->transition_table[NX_CHAR_EPSILON1]);
            nx_set_add(&new_ss, s->transition_table[NX_CHAR_EPSILON2]);
        } else {
            nx_set_add(&new_ss, s->transition_table[b]);
        }
    }
    return new_ss;
}

static struct nx_set nx_match_transition(const struct nx * nx, enum nx_char b, struct nx_set ss) {
    if (nx_set_isempty(&ss)) {
        return ss;
    }

    struct nx_set new_ss = nx_match_transition2(nx, b, ss);
    while (true) {
        ss = new_ss;
        const struct nx_set epsilon_ss = nx_match_transition2(nx, NX_CHAR_EPSILON1, ss);
        nx_set_orequal(&new_ss, &epsilon_ss);
        if (memcmp(&ss, &new_ss, sizeof(ss)) == 0) {
            break;
        }
    }
    return new_ss;
}

static int nx_match_fuzzy(const struct nx * nx, const enum nx_char * buffer, size_t bi, struct nx_set ss,
                          size_t n_errors) {
    if (nx_set_test(&ss, TRANSITION_SUCCESS)) {
        return 0;
    }
    struct nx_set err_ss = {0};
    while (true) {
        struct nx_set next_ss = nx_match_transition(nx, buffer[bi], ss);
        struct nx_set next_err_ss = nx_match_transition(nx, buffer[bi], err_ss);
        size_t next_bi = bi + 1;
        if (nx_set_test(&next_ss, TRANSITION_SUCCESS)) {
            return 0;
        }
        if (nx_set_test(&next_err_ss, TRANSITION_SUCCESS)) {
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

        if (nx_set_isempty(&next_ss)) {
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

    struct nx_set ss = nx_match_transition(nx, NX_CHAR_EPSILON1, NX_SET_START);
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
    struct nx * nx = nx_compile("he?a?z?l+?oworld");
    // struct nx * nx = nx_compile("(thing|hello|asdf|world)*");
    // struct nx * nx = nx_compile("helloworld");
    nx_print_nfa(nx);
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
