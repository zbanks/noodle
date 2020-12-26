#include "nx.h"

static const struct nx_set NX_SET_START = {{1}};

bool nx_set_test(const struct nx_set * s, size_t i) {
    if (i >= NX_SET_SIZE + 1) {
        return false;
    }
    return (s->xs[i / 64] & (1ul << (i % 64))) != 0;
}

#define EMPTYBIT // ~5% speedup when NX_STATE_MAX=255
bool nx_set_isempty(const struct nx_set * s) {
#ifdef EMPTYBIT
    return (s->xs[NX_SET_SIZE / 64] & (1ul << 63u)) == 0;
#else
    for (size_t i = 0; i < NX_SET_ARRAYLEN; i++) {
        if (s->xs[i]) {
            return false;
        }
    }
    return true;
#endif
}

bool nx_set_add(struct nx_set * s, size_t i) {
    if (i >= NX_SET_SIZE) {
        return false;
    }
    if (nx_set_test(s, i)) {
        return false;
    }
    s->xs[i / 64] |= (1ul << (i % 64));
#ifdef EMPTYBIT
    s->xs[NX_SET_SIZE / 64] |= (1ul << 63u);
#endif
    return true;
}

void nx_set_orequal(struct nx_set * restrict s, const struct nx_set * restrict t) {
    for (size_t i = 0; i < NX_SET_ARRAYLEN; i++) {
        s->xs[i] |= t->xs[i];
    }
}

bool nx_set_intersect(const struct nx_set * s, const struct nx_set * t) {
    for (size_t i = 0; i < NX_SET_ARRAYLEN; i++) {
        uint64_t overlap = s->xs[i] & t->xs[i];
#ifdef EMPTYBIT
        if (i == NX_SET_ARRAYLEN - 1) {
            overlap &= ~(1ul << 63u);
        }
#endif
        if (overlap) {
            return true;
        }
    }
    return false;
}

const char * nx_set_debug(const struct nx_set * s) {
    static char buffer[NX_SET_SIZE * 6];
    char * b = buffer;
    if (nx_set_isempty(s)) {
        return "(empty)";
    }
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

_Static_assert(_NX_CHAR_MAX < 32, "Unexpectedly large enum nx_char");

enum {
    STATE_SUCCESS = NX_SET_SIZE - 1,
    STATE_FAILURE = (uint16_t)-1,
};

_Static_assert(NX_STATE_MAX < UINT16_MAX, "NX_STATE_MAX too big for a uint16_t");
_Static_assert(_NX_CHAR_MAX <= 32, "_NX_CHAR_MAX cannot fit in a uint32_t");

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
        return NX_CHAR_OTHER;
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
    case NX_CHAR_OTHER:
        return '-';
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

void nx_char_translate(const char * input, enum nx_char * output, size_t output_size) {
    output[0] = NX_CHAR_SPACE;
    for (size_t i = 1;; i++) {
        ASSERT(i + 1 < output_size);
        output[i] = nx_char(*input++);
        if (output[i] == NX_CHAR_END) {
            output[i] = NX_CHAR_SPACE;
            output[i + 1] = NX_CHAR_END;
            break;
        }
    }
}

static void nx_nfa_debug(const struct nx * nx) {
    LOG("NX NFA: %zu states", nx->n_states);
    for (size_t i = 0; i < nx->n_states; i++) {
        const struct nx_state * s = &nx->states[i];

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
        if (!nx_set_isempty(&s->epsilon_states)) {
            printf("* -> %s", nx_set_debug(&s->epsilon_states));
        }
        // printf("\t [%#x->%u %#x->%u]",
        //        s->char_bitset[0], s->next_state[0],
        //        s->char_bitset[1], s->next_state[1]);
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
    enum nx_char implicit_char_bitset = 0;
    if (nx->implicit_spaces) {
        implicit_char_bitset |= nx_char_bit(NX_CHAR_SPACE);
    }
    if (nx->implicit_other) {
        implicit_char_bitset |= nx_char_bit(NX_CHAR_OTHER);
    }

    ssize_t consumed_characters = 0;
    size_t previous_initial_state = STATE_FAILURE;
    size_t subexpression_initial_state = nx->n_states;
    size_t subexpression_final_state = STATE_FAILURE;
    for (const char * c = subexpression;; c++) {
        struct nx_state * s = &nx->states[nx->n_states];
        ASSERT(nx->n_states < NX_STATE_MAX);

        enum nx_char nc = nx_char(*c);
        switch (*c) {
        case '\\':
        case '^':
        case '$':
        case ' ':
            break;
        case ')':
            if (subexpression_final_state != STATE_FAILURE) {
                LOG("Subexpression %zu", subexpression_final_state);
                nx->states[subexpression_final_state].next_state[0] = (uint16_t)(nx->n_states);
            }
            return consumed_characters;
        case '\0':
            s->next_state[0] = STATE_SUCCESS;
            s->char_bitset[0] = nx_char_bit(NX_CHAR_END);

            // NB: This technically matches an arbitrary number of spaces (or none at all)
            s->next_state[1] = (uint16_t)(nx->n_states);
            s->char_bitset[1] = nx_char_bit(NX_CHAR_SPACE) | implicit_char_bitset;

            nx->n_states++;

            if (subexpression_final_state != STATE_FAILURE) {
                LOG("Subexpression %zu", subexpression_final_state);
                nx->states[subexpression_final_state].next_state[0] = (uint16_t)(nx->n_states);
            }
            return consumed_characters;
        case 'A' ... 'Z':
        case 'a' ... 'z':
            s->next_state[0] = (uint16_t)(nx->n_states + 1);
            s->char_bitset[0] = nx_char_bit(nc);

            if (implicit_char_bitset) {
                s->next_state[1] = (uint16_t)(nx->n_states);
                s->char_bitset[1] = implicit_char_bitset;
            }

            previous_initial_state = nx->n_states;
            nx->n_states++;
            break;
        case '_': // Explicit space
            s->next_state[0] = (uint16_t)(nx->n_states + 1);
            s->char_bitset[0] = nx_char_bit(NX_CHAR_SPACE);

            if (implicit_char_bitset) {
                s->next_state[1] = (uint16_t)(nx->n_states);
                s->char_bitset[1] = implicit_char_bitset;
            }

            previous_initial_state = nx->n_states;
            nx->n_states++;
            break;
        case '-': // Explicit other
            s->next_state[0] = (uint16_t)(nx->n_states + 1);
            s->char_bitset[0] = nx_char_bit(NX_CHAR_OTHER);

            if (implicit_char_bitset) {
                s->next_state[1] = (uint16_t)(nx->n_states);
                s->char_bitset[1] = implicit_char_bitset;
            }

            previous_initial_state = nx->n_states;
            nx->n_states++;
            break;
        case '.':
            s->next_state[0] = (uint16_t)(nx->n_states + 1);
            for (enum nx_char j = NX_CHAR_OTHER; j <= NX_CHAR_Z; j++) {
                s->char_bitset[0] |= nx_char_bit(j);
            }

            if (implicit_char_bitset) {
                s->next_state[1] = (uint16_t)(nx->n_states);
                s->char_bitset[1] = implicit_char_bitset;
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

            s->next_state[0] = (uint16_t)(nx->n_states + 1);
            s->char_bitset[0] = 0;

            while (*c != ']' && *c != '\0') {
                if (nx_char(*c) >= NX_CHAR_OTHER && nx_char(*c) <= NX_CHAR_Z) {
                    s->char_bitset[0] |= nx_char_bit(nx_char(*c));
                } else if (*c != ' ') {
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

            if (implicit_char_bitset) {
                s->next_state[1] = (uint16_t)(nx->n_states);
                s->char_bitset[1] = implicit_char_bitset;
            }

            previous_initial_state = nx->n_states;
            nx->n_states++;
            break;
        case '*': {
            if (previous_initial_state == STATE_FAILURE) {
                LOG("nx parse error: '%c' without preceeding group", *c);
                return -1;
            }

            struct nx_state * epsilon_s = nx_state_insert(nx, previous_initial_state++);
            if (previous_initial_state < subexpression_final_state && subexpression_final_state != STATE_FAILURE) {
                subexpression_final_state++;
            }
            epsilon_s->next_state[0] = (uint16_t)previous_initial_state;
            epsilon_s->char_bitset[0] = nx_char_bit(NX_CHAR_EPSILON);
            epsilon_s->next_state[1] = (uint16_t)(nx->n_states + 1);
            epsilon_s->char_bitset[1] = nx_char_bit(NX_CHAR_EPSILON);

            s = &nx->states[nx->n_states];
            s->next_state[0] = (uint16_t)previous_initial_state;
            s->char_bitset[0] = nx_char_bit(NX_CHAR_EPSILON);
            s->next_state[1] = (uint16_t)(nx->n_states + 1);
            s->char_bitset[1] = nx_char_bit(NX_CHAR_EPSILON);

            nx->n_states++;
            break;
        }
        case '+':
            if (previous_initial_state == STATE_FAILURE) {
                LOG("nx parse error: '%c' without preceeding group", *c);
                return -1;
            }

            s = &nx->states[nx->n_states];
            s->next_state[0] = (uint16_t)previous_initial_state;
            s->char_bitset[0] = nx_char_bit(NX_CHAR_EPSILON);
            s->next_state[1] = (uint16_t)(nx->n_states + 1);
            s->char_bitset[1] = nx_char_bit(NX_CHAR_EPSILON);

            nx->n_states++;
            break;
        case '?': {
            if (previous_initial_state == STATE_FAILURE) {
                LOG("nx parse error: '%c' without preceeding group", *c);
                return -1;
            }

            struct nx_state * epsilon_s = nx_state_insert(nx, previous_initial_state++);
            if (previous_initial_state < subexpression_final_state && subexpression_final_state != STATE_FAILURE) {
                subexpression_final_state++;
            }
            epsilon_s->next_state[0] = (uint16_t)previous_initial_state;
            epsilon_s->char_bitset[0] = nx_char_bit(NX_CHAR_EPSILON);
            epsilon_s->next_state[1] = (uint16_t)(nx->n_states);
            epsilon_s->char_bitset[1] = nx_char_bit(NX_CHAR_EPSILON);

            break;
        }
        case '{':
            if (previous_initial_state == STATE_FAILURE) {
                LOG("nx parse error: '%c' without preceeding group", *c);
                return -1;
            }
            c++;
            consumed_characters++;
            size_t min_repeat = 0;
            size_t max_repeat = 0;
            bool seen_comma = false;

            while (*c != '}' && *c != '\0') {
                if (*c == ',') {
                    seen_comma = true;
                } else if (*c >= '0' && *c <= '9') {
                    size_t *n = seen_comma ? &max_repeat : &min_repeat;
                    *n = (*n * 10u) + (size_t) (*c - '0');
                    if (*n > NX_SET_SIZE) {
                        LOG("Parse error; values in {...} group too large");
                        return -1;
                    }
                } else if (*c != ' ') {
                    LOG("Parse error; invalid character '%c' in {...} group", *c);
                    return -1;
                }
                c++;
                consumed_characters++;
            }
            if (*c == '\0') {
                LOG("Parse error; unterminated {");
                return -1;
            }
            if (seen_comma && max_repeat != 0 && (min_repeat > max_repeat)) {
                LOG("Parse error: {%zu,%zu} is invalid, min must be less than max", min_repeat, max_repeat);
                return -1;
            }
            if (min_repeat <= 0 && max_repeat <= 0) {
                LOG("Parse error: {%zu,%zu} is invalid", min_repeat, max_repeat);
                return -1;
            }
            if (!seen_comma) {
                max_repeat = min_repeat;
            }

            ASSERT(nx->n_states >= 1);
            size_t copy_start = previous_initial_state;
            size_t copy_end = nx->n_states - 1;
            size_t copy_count = max_repeat == 0 ? min_repeat : max_repeat;
            ASSERT(copy_count > 1);
            ASSERT(copy_start <= copy_end);

            size_t initial_state;
            for (size_t j = 1; j < copy_count; j++) {
                initial_state = nx->n_states;
                if (j >= min_repeat) {
                    // Add a `?`-like state
                    ASSERT(max_repeat != 0);
                    s = &nx->states[nx->n_states];
                    s->next_state[0] = (uint16_t)(nx->n_states + 1);
                    s->char_bitset[0] = nx_char_bit(NX_CHAR_EPSILON);
                    s->next_state[1] = (uint16_t)(nx->n_states + (copy_end - copy_start) + 2);
                    s->char_bitset[1] = nx_char_bit(NX_CHAR_EPSILON);
                    nx->n_states++;
                }
                for (size_t k = copy_start; k <= copy_end; k++) {
                    s = &nx->states[nx->n_states];
                    *s = nx->states[k];
                    for (size_t b = 0; b < NX_BRANCH_COUNT; b++) {
                        if (s->next_state[b] >= copy_start && s->next_state[b] <= copy_end + 1 && s->char_bitset[b] != 0) {
                            ASSERT(nx->n_states > k);
                            s->next_state[b] = (uint16_t) (s->next_state[b] + nx->n_states - k);
                        }
                    }

                    nx->n_states++;
                    ASSERT(nx->n_states < NX_STATE_MAX);
                }
            }
            if (max_repeat == 0) {
                // Add a `+`-like state
                s = &nx->states[nx->n_states];
                s->next_state[0] = (uint16_t)initial_state;
                s->char_bitset[0] = nx_char_bit(NX_CHAR_EPSILON);
                s->next_state[1] = (uint16_t)(nx->n_states + 1);
                s->char_bitset[1] = nx_char_bit(NX_CHAR_EPSILON);
                nx->n_states++;
            }
            ASSERT(nx->n_states < NX_STATE_MAX);
            break;
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

            epsilon_s->next_state[0] = (uint16_t)(subexpression_initial_state + 1);
            epsilon_s->char_bitset[0] = nx_char_bit(NX_CHAR_EPSILON);
            epsilon_s->next_state[1] = (uint16_t)(nx->n_states);
            epsilon_s->char_bitset[1] = nx_char_bit(NX_CHAR_EPSILON);

            if (subexpression_final_state == STATE_FAILURE) {
                subexpression_final_state = nx->n_states;
                s = &nx->states[subexpression_final_state];
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
            LOG("Invalid character in nx expression: '%c'", *c);
            return -1;
        }

        consumed_characters++;
    }
}

struct nx * nx_compile(const char * expression) {
    NONNULL(expression);

    struct nx * nx = NONNULL(calloc(1, sizeof(*nx)));
    nx->expression = NONNULL(strdup(expression));
    nx->implicit_spaces = (strchr(expression, '_') == NULL);
    nx->implicit_other = (strchr(expression, '-') == NULL);

    ssize_t rc = nx_compile_subexpression(nx, nx->expression);
    if (rc < 0) {
        goto fail;
    }
    ASSERT(rc == (ssize_t)strlen(nx->expression));

    // Calculate epsilon transitions
    for (size_t i = 0; i < nx->n_states; i++) {
        struct nx_state * s = &nx->states[i];
        struct nx_set next_ss = s->epsilon_states;
        for (size_t j = 0; j < NX_BRANCH_COUNT; j++) {
            if (nx_char_bit(NX_CHAR_EPSILON) & s->char_bitset[j]) {
                nx_set_add(&next_ss, s->next_state[j]);
            }
        }
        while (true) {
            struct nx_set ss = next_ss;
            for (size_t si = 0; si < nx->n_states; si++) {
                if (!nx_set_test(&next_ss, si)) {
                    continue;
                }
                const struct nx_state * s2 = &nx->states[si];

                for (size_t j = 0; j < NX_BRANCH_COUNT; j++) {
                    if (nx_char_bit(NX_CHAR_EPSILON) & s2->char_bitset[j]) {
                        nx_set_add(&ss, s2->next_state[j]);
                    }
                }
            }
            if (memcmp(&ss, &next_ss, sizeof(ss)) == 0) {
                break;
            }
            nx_set_orequal(&next_ss, &ss);
        }
        s->epsilon_states = next_ss;
    }
    for (size_t i = 0; i < nx->n_states; i++) {
        struct nx_state * s = &nx->states[i];
        for (size_t j = 0; j < NX_BRANCH_COUNT; j++) {
            if (s->char_bitset[j] == nx_char_bit(NX_CHAR_EPSILON)) {
                s->char_bitset[j] = 0;
                s->next_state[j] = 0;
            }
        }
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

// Return thes et of possible ending states, from starting at one of the `start_states`
// and consuming one character from `char_bitset`.
static struct nx_set nx_match_transition(const struct nx * nx, uint32_t char_bitset, struct nx_set start_states) {
    // Start with an empty result set.
    struct nx_set end_states = {0};

    // If there are no valid `start_states`, there are no valid end states!
    if (nx_set_isempty(&start_states)) {
        return end_states;
    }

    // Iterate over all states contained in `start_states`,
    // looking for non-epsilon transitions
    for (size_t si = 0; si < nx->n_states; si++) {
        if (!nx_set_test(&start_states, si)) {
            continue;
        }

        // For each state, check which edges are accessible with `char_bitset`
        const struct nx_state * s = &nx->states[si];
        // TODO: Can this for loop be removed? s->char_bitset[1] should be 0 at this point
        for (size_t j = 0; j < NX_BRANCH_COUNT; j++) {
            if (char_bitset & s->char_bitset[j]) {
                // If an edge does match `char_bitset`, add it to the result set
                nx_set_add(&end_states, s->next_state[j]);
            }
        }
    }

    // Now that `end_states` contains all non-epsilon transitions, fill
    // in the possible *epsilon* transitions. Iterate over each state in `end_states`
    for (size_t si = 0; si < nx->n_states; si++) {
        if (!nx_set_test(&end_states, si)) {
            continue;
        }

        // `epsilon_states` is pre-computed to cover all states reachable by
        // an arbitrary number of epsilon transitions from the given state.
        // This allows this segment to run in constant time.
        const struct nx_state * s = &nx->states[si];
        nx_set_orequal(&end_states, &s->epsilon_states);
    }

    return end_states;
}

struct nx_set nx_match_partial(const struct nx * nx, const enum nx_char * buffer, uint16_t initial_state) {
    // Start with an initial `state_set` containing only `initial_state`
    struct nx_set state_set = {0};
    nx_set_add(&state_set, initial_state);

    // Add all `epsilon_states`, which are the states reachable from `initial_state`
    // without consuming a character from `buffer`
    nx_set_orequal(&state_set, &nx->states[initial_state].epsilon_states);

    for (size_t bi = 0; buffer[bi] != NX_CHAR_END; bi++) {
        // Consume 1 character from the buffer and compute the set of possible resulting states
        state_set = nx_match_transition(nx, nx_char_bit(buffer[bi]), state_set);

        // We can terminate early if there are no possible valid states
        if (nx_set_isempty(&state_set)) {
            break;
        }
    }

    // Return the set of possible result states from consuming every character in `buffer`
    return state_set;
}

// Perform a fuzzy match against an NX NFA.
// From a given starting `state_set`, return the number of changes required for `buffer` to match `nx`.
// Returns `-1` if the number of errors would exceed `n_errors`, or `0` on an exact match.
// Can be used for exact matches by setting `n_errors` to `0`.
static int nx_match_fuzzy(const struct nx * nx, const enum nx_char * buffer, struct nx_set state_set, size_t n_errors) {
    // If the initial `state_set` is already a match, we're done!
    if (nx_set_test(&state_set, STATE_SUCCESS)) {
        return 0;
    }

    // `LETTER_SET` contains every valid letter (A-Z, no metacharacters). It is initialized once.
    // It is used to represent which items can be added to `buffer` during fuzzy matching.
    static uint32_t LETTER_SET = 0;
    if (LETTER_SET == 0) {
        for (enum nx_char j = NX_CHAR_A; j <= NX_CHAR_Z; j++) {
            LETTER_SET |= nx_char_bit(j);
        }
    }

    // Keep track of which states are reachable with *exactly* 1 error, initially empty
    struct nx_set error_state_set = {0};

    // Iterate over the characters in `buffer` (exactly 1 character per iteration)
    while (true) {
        // Consume 1 character from the buffer and compute the set of possible resulting states
        struct nx_set next_state_set = nx_match_transition(nx, nx_char_bit(*buffer), state_set);

        // The same, but with the set of states reachable with exactly 1 error
        struct nx_set next_error_set = nx_match_transition(nx, nx_char_bit(*buffer), error_state_set);

        // If SUCCESS is reachable, it's a match, we're done!
        if (nx_set_test(&next_state_set, STATE_SUCCESS)) {
            // The NFA should be constructed so that SUCCESS is only reachable with an END character
            ASSERT(*buffer == NX_CHAR_END);
            return 0;
        }

        // If SUCCESS is reachable with exactly 1 error, it was _almost_ a match. Return 1 to mark the error.
        if (nx_set_test(&next_error_set, STATE_SUCCESS)) {
            ASSERT(*buffer == NX_CHAR_END);
            return 1;
        }

        // If we are performing a fuzzy match, then expand `next_error_set` by adding all states
        // reachable from `state_set` *but* with a 1-character change to `buffer`
        if (n_errors > 0) {
            // We can only delete/change characters if they aren't the terminating END
            if (*buffer != NX_CHAR_END) {
                // Deletion: skip over using `*buffer` to do a transition
                nx_set_orequal(&next_error_set, &state_set);

                // Change: use a different char (any letter) to do a transition instead of `*buffer`
                struct nx_set es = nx_match_transition(nx, LETTER_SET, state_set);
                nx_set_orequal(&next_error_set, &es);
            }

            // XXX: This doesn't handle two inserted letters in a row
            // Insertion: insert a char (any letter) before *buffer...
            struct nx_set es = nx_match_transition(nx, LETTER_SET, state_set);
            // ...then use *buffer
            es = nx_match_transition(nx, nx_char_bit(*buffer), es);
            nx_set_orequal(&next_error_set, &es);
        }

        // If there are no possible states after consuming `*buffer`, that's not looking good.
        // The best we can do is seeing if there is a fuzzy match
        if (nx_set_isempty(&next_state_set)) {
            if (n_errors > 0) {
                int rc = nx_match_fuzzy(nx, buffer + 1, next_error_set, n_errors - 1);
                if (rc >= 0) {
                    // There was a fuzzy match! Because we used `next_error_set`, increment the number of errors
                    return rc + 1;
                }
            }

            // No fuzzy match
            return -1;
        }

        // The NFA should be constructed such that `*buffer` can't be `NX_CHAR_END` here.
        // If it were, either `next_state_set` would contain SUCCESS or be empty, and we would have returned.
        ASSERT(*buffer != NX_CHAR_END);

        // Advance the buffer to the next character, and shift the state sets
        buffer++;
        state_set = next_state_set;
        error_state_set = next_error_set;
    }
}

int nx_match(const struct nx * nx, const char * input, size_t n_errors) {
    enum nx_char buffer[256];
    nx_char_translate(input, buffer, 256);

    // `epsilon_states` are accounted for *after* "normal" states in `nx_match_transition`
    // Therefore it is important to include them here for correctness
    struct nx_set ss = nx->states[0].epsilon_states;
    nx_set_orequal(&ss, &NX_SET_START);

    return nx_match_fuzzy(nx, buffer, ss, n_errors);
}

void nx_test(void) {
    // struct nx * nx = nx_compile("([^asdfzyxwv]el([lw]o)+r[lheld]*)+");
    // struct nx * nx = nx_compile("he?a?z?l+?oworld");
    struct nx * nx = nx_compile("(thing|hello|asdf|world|a?b?c?d?e?)+");
    // struct nx * nx = nx_compile("helloworld");
    const char * s[] = {
        "helloworld",
        "hello",
        "helloworldhello",
        "helloworldhelloworld",
        "h e l l o w o r l d",
        "helloworl",
        "helloworlda",
        "heloworld",
        "hellloworld",
        "hellaworld",
        "aaaaasdfawjeojworkld",
        "heoworld",
        "elloworld",
        "hloworld",
        NULL,
    };
    // LOG("rc = %d", nx_match(nx, "hellowor", 0));
    for (size_t i = 0; s[i] != NULL; i++) {
        int rc = nx_match(nx, s[i], 3);
        LOG("> \"%s\": %d", s[i], rc);

        enum nx_char buffer[256];
        nx_char_translate(s[i], buffer, 256);
        struct nx_set ps = nx_match_partial(nx, buffer, 0);
        LOG("Partial: %s", nx_set_debug(&ps));
    }
}
