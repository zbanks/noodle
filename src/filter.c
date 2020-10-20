#include "filter.h"
#include <regex.h>

int filter_regex(const char * regex, const struct wordset * src, struct wordset * dst) {
    char regex_modified[1024];
    if (strlen(regex) + 1 > sizeof(regex_modified)) {
        return -1;
    }
    snprintf(regex_modified, sizeof(regex_modified) - 1, "^%s$", regex);

    regex_t preg;
    int rc = regcomp(&preg, regex_modified, REG_EXTENDED | REG_ICASE | REG_NOSUB);
    if (rc != 0) {
        return -1;
    }

    for (size_t i = 0; i < src->words_count; i++) {
        if (regexec(&preg, str_str(&src->words[i]->canonical), 0, NULL, 0) == 0) {
            wordset_add(dst, src->words[i]);
        }
    }

    regfree(&preg);
    return 0;
}

void filter_anagram(const char * letters, const struct wordset * src, struct wordset * dst) {
    struct word w;
    word_init(&w, letters, 0);
    const char * sorted_letters = str_str(&w.sorted);
    for (size_t i = 0; i < src->words_count; i++) {
        if (strcmp(sorted_letters, str_str(&src->words[i]->sorted)) == 0) {
            wordset_add(dst, src->words[i]);
        }
    }
    word_term(&w);
}

static bool difference_size_less_than(const char * superset, const char * subset, size_t max_size) {
    ASSERT(superset != NULL && subset != NULL);
    size_t size = 0;
    while (*subset != '\0') {
        if (*superset == '\0') {
            // The rest of subset is not in superset
            return false;
        }
        if (*superset == *subset) {
            superset++;
            subset++;
        } else if (*superset > *subset) {
            // There is a letter in subset not in superset
            return false;
        } else {
            // There is a letter in superset not in subset
            ASSERT(*superset < *subset);
            superset++;
            size++;
            if (size > max_size) {
                return false;
            }
        }
    }
    if (size + strlen(superset) > max_size) {
        return false;
    }
    return true;
}

void filter_subanagram(const char * letters, const struct wordset * src, struct wordset * dst) {
    struct word w;
    word_init(&w, letters, 0);
    const char * sorted_letters = str_str(&w.sorted);
    for (size_t i = 0; i < src->words_count; i++) {
        if (difference_size_less_than(sorted_letters, str_str(&src->words[i]->sorted), -1ul)) {
            wordset_add(dst, src->words[i]);
        }
    }
    word_term(&w);
}

void filter_superanagram(const char * letters, const struct wordset * src, struct wordset * dst) {
    struct word w;
    word_init(&w, letters, 0);
    const char * sorted_letters = str_str(&w.sorted);
    for (size_t i = 0; i < src->words_count; i++) {
        if (difference_size_less_than(str_str(&src->words[i]->sorted), sorted_letters, -1ul)) {
            wordset_add(dst, src->words[i]);
        }
    }
    word_term(&w);
}

void filter_transdelete(size_t n, const char * letters, const struct wordset * src, struct wordset * dst) {
    struct word w;
    word_init(&w, letters, 0);
    const char * sorted_letters = str_str(&w.sorted);
    for (size_t i = 0; i < src->words_count; i++) {
        const char * item = str_str(&src->words[i]->sorted);
        if (strlen(item) + n != strlen(sorted_letters)) {
            continue;
        }
        if (difference_size_less_than(sorted_letters, item, n)) {
            wordset_add(dst, src->words[i]);
        }
    }
    word_term(&w);
}

void filter_transadd(size_t n, const char * letters, const struct wordset * src, struct wordset * dst) {
    struct word w;
    word_init(&w, letters, 0);
    const char * sorted_letters = str_str(&w.sorted);
    for (size_t i = 0; i < src->words_count; i++) {
        const char * item = str_str(&src->words[i]->sorted);
        if (strlen(item) != strlen(sorted_letters) + n) {
            continue;
        }
        if (difference_size_less_than(item, sorted_letters, n)) {
            wordset_add(dst, src->words[i]);
        }
    }
    word_term(&w);
}

void filter_bank(const char * letters, const struct wordset * src, struct wordset * dst) {
    for (size_t i = 0; i < src->words_count; i++) {
        const char * s = str_str(&src->words[i]->sorted);
        for (; *s != '\0'; s++) {
            if (strchr(letters, *s) == NULL) {
                break;
            }
        }
        if (*s == '\0') {
            wordset_add(dst, src->words[i]);
        }
    }
}

//
const char * const filter_type_names[] = {
        [FILTER_REGEX] = "regex",
        [FILTER_ANAGRAM] = "anagram",
        [FILTER_SUBANAGRAM] = "subanagram",
        [FILTER_SUPERANAGRAM] = "superanagram",
        [FILTER_TRANSADD] = "transadd",
        [FILTER_TRANSDELETE] = "transdelete",
        [FILTER_BANK] = "bank",
};

void filter_init(struct filter * f, enum filter_type type, size_t n, const char * str) {
    f->type = type;
    f->arg_n = n;
    f->arg_str = NONNULL(strdup(str));

    if (f->arg_n != -1ul) {
        snprintf(f->name, sizeof(f->name), "%s %zu: %s", filter_type_names[f->type], f->arg_n, f->arg_str);
    } else {
        snprintf(f->name, sizeof(f->name), "%s: %s", filter_type_names[f->type], f->arg_str);
    }
}

int filter_parse(struct filter * f, const char * spec) {
    regex_t preg;
    const char * regex = "^\\s*([a-z]+)\\s*([0-9]*)\\s*:\\s*(\\S+)\\s*$";
    ASSERT(regcomp(&preg, regex, REG_EXTENDED | REG_ICASE) == 0);

    regmatch_t matches[4];
    int rc = regexec(&preg, spec, 4, matches, 0);
    if (rc != 0) {
        LOG("filter does not match regex '%s' !~ /%s/", spec, regex);
        return -1;
    }

    size_t size = strlen(spec) + 1;
    char * buffer = NONNULL(calloc(1, size));

    memset(buffer, 0, size);
    memcpy(buffer, &spec[matches[1].rm_so], (size_t)(matches[1].rm_eo - matches[1].rm_so));
    enum filter_type type = _FILTER_TYPE_MAX;
    for (size_t i = 0; i < _FILTER_TYPE_MAX; i++) {
        if (strcmp(filter_type_names[i], buffer) == 0) {
            type = i;
            break;
        }
    }
    if (type == _FILTER_TYPE_MAX) {
        LOG("Invalid filter type '%s'", buffer);
        goto fail;
    }

    memset(buffer, 0, size);
    memcpy(buffer, &spec[matches[2].rm_so], (size_t)(matches[2].rm_eo - matches[2].rm_so));
    size_t n = -1ul;
    if (*buffer != '\0') {
        errno = 0;
        n = strtoul(buffer, NULL, 10);
        if (errno != 0) {
            LOG("Invalid n argument '%s'", buffer);
            goto fail;
        }
    }

    memset(buffer, 0, size);
    memcpy(buffer, &spec[matches[3].rm_so], (size_t)(matches[3].rm_eo - matches[3].rm_so));
    if (*buffer == '\0') {
        LOG("Missing str argument");
        goto fail;
    }

    filter_init(f, type, n, buffer);
    free(buffer);
    return 0;

fail:
    free(buffer);
    return -1;
}

int filter_apply(struct filter * f, struct wordset * input) {
    wordset_init(&f->output, f->name);
    int rc = 0;
    switch (f->type) {
    case FILTER_REGEX:
        rc = filter_regex(f->arg_str, input, &f->output);
        break;
    case FILTER_ANAGRAM:
        filter_anagram(f->arg_str, input, &f->output);
        break;
    case FILTER_SUBANAGRAM:
        filter_subanagram(f->arg_str, input, &f->output);
        break;
    case FILTER_SUPERANAGRAM:
        filter_superanagram(f->arg_str, input, &f->output);
        break;
    case FILTER_TRANSADD:
        filter_transadd(f->arg_n, f->arg_str, input, &f->output);
        break;
    case FILTER_TRANSDELETE:
        filter_transdelete(f->arg_n, f->arg_str, input, &f->output);
        break;
    case FILTER_BANK:
        filter_bank(f->arg_str, input, &f->output);
        break;
    default:
        ASSERT(false);
    }
    return rc;
}

void filter_term(struct filter * f) {
    free(f->arg_str);
    wordset_term(&f->output);
}
