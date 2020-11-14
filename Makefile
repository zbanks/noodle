TARGETS = libnoodle.so noodle noodle_ffi.cpython-39-x86_64-linux-gnu.so

CC=gcc

CFLAGS += -std=c11 -D_DEFAULT_SOURCE
CFLAGS += -Wall -Wextra -Wconversion -Werror
CFLAGS += -ggdb3
CFLAGS += -O3
CFLAGS += -flto
CFLAGS += -fPIC -fvisibility=hidden
CFLAGS += -Isrc/
CFLAGS += -DDEBUG
LFLAGS = -Wl,-z,origin '-Wl,-rpath=$$ORIGIN'

LIB_SRCS = \
	src/anatree.c \
	src/filter.c \
	src/nx.c \
	src/nx_combo.c \
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
	$(CC) -MM $(CFLAGS) src/$*.c > build/$*.d

libnoodle.so: $(OBJECTS) | $(DEPS)
	$(CC) $^ -shared $(CFLAGS) $(LFLAGS) -o $@

noodle: src/main.o libnoodle.so | $(DEPS)
	$(CC) $^ $(CFLAGS) $(LFLAGS) -o $@

noodle_ffi.cpython-39-x86_64-linux-gnu.so: build_cffi.py | libnoodle.so
	python3.9 $< && cp build/$@ $@

.PHONY: format clean all
format:
	clang-format -i src/*.c src/*.h
	black -q .

clean:
	-rm -rf build $(TARGETS)

.PHONY: all
all: $(TARGETS)

.DEFAULT_GOAL = all
