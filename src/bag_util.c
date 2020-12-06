#include "bag_util.h"

bool bag_difference_size_less_than(const char * superset, const char * subset, size_t max_size) {
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

bool bag_subtract_into(const char * superset, const char * subset, char * output) {
    ASSERT(superset != NULL && subset != NULL);
    while (*superset != '\0') {
        if (*subset == '\0') {
            // The rest of subset is not in superset
            // Copy the remaining letters into the output
            while (*superset != '\0') {
                *output++ = *superset++;
            }
            break;
        }
        if (*superset == *subset) {
            // The letter is in both subset & superset
            superset++;
            subset++;
        } else if (*superset > *subset) {
            // There is a letter in subset not in superset
            return false;
        } else {
            // There is a letter in superset not in subset
            ASSERT(*superset < *subset);
            *output++ = *superset++;
        }
    }
    if (*subset != '\0') {
        return false;
    }
    *output = '\0';
    return true;
}
