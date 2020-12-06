CC=gcc
PYTHON=python3.9

TARGET_CFFI_LIB = noodle_ffi$(shell $(PYTHON) -c 'import importlib.machinery as m; print(m.EXTENSION_SUFFIXES[0])')
TARGETS = libnoodle.so noodle $(TARGET_CFFI_LIB)

CFLAGS += -std=c11 -D_DEFAULT_SOURCE
CFLAGS += -Wall -Wextra -Wconversion -Werror
CFLAGS += -ggdb3
CFLAGS += -O3 -flto
CFLAGS += -fwrapv
CFLAGS += -fPIC -fvisibility=hidden
CFLAGS += -Isrc/
CFLAGS += -DDEBUG
LFLAGS = -Wl,-z,origin '-Wl,-rpath=$$ORIGIN'

LIB_SRCS = \
	src/anatree.c \
	src/anagram_slow.c \
	src/bag_util.c \
	src/error.c \
	src/filter.c \
	src/nx.c \
	src/nx_combo.c \
	src/time_util.c \
	src/word.c \
	src/wordlist.c \

# Disable built-in rules
MAKEFLAGS += --no-builtin-rules
.SUFFIXES:

# Create a folder for intermediate build artifacts
$(shell mkdir -p build)

OBJECTS = $(patsubst src/%.c,build/%.o,$(LIB_SRCS))
DEPS = $(OBJECTS:.o=.d) build/main.d
-include $(DEPS)

build/%.o: src/%.c
	$(CC) -c $(CFLAGS) src/$*.c -o build/$*.o

build/%.d: src/%.c
	@$(CC) -MM $(CFLAGS) src/$*.c > build/$*.d

libnoodle.so: $(OBJECTS)
	$(CC) $^ -shared $(CFLAGS) $(LFLAGS) -o $@

noodle: build/main.o libnoodle.so
	$(CC) $^ $(CFLAGS) $(LFLAGS) -o $@

$(TARGET_CFFI_LIB): build_cffi.py | libnoodle.so
	$(PYTHON) $< && cp build/$@ $@

.PHONY: format clean all
format:
	clang-format -i src/*.c src/*.h
	black -q .

clean:
	-rm -rf build __pycache__ $(TARGETS)

.PHONY: all
all: $(TARGETS)

.DEFAULT_GOAL = all
