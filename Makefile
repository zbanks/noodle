TARGETS = libnoodle.so noodle

CC=gcc

CFLAGS += -std=c11 -D_DEFAULT_SOURCE
CFLAGS += -Wall -Wextra -Wconversion -Werror
CFLAGS += -ggdb3
CFLAGS += -O3
CFLAGS += -flto
CFLAGS += -fPIC -fvisibility=hidden
CFLAGS += -Isrc/
CFLAGS += -DDEBUG
LFLAGS = -Wl,-rpath,"."

SRCS = \
	src/anatree.c \
	src/filter.c \
	src/nx.c \
	src/nx_combo.c \
	src/word.c \
	src/wordlist.c \

$(shell mkdir -p build)
OBJECTS = $(patsubst src/%.c,build/%.o,$(SRCS))
DEPS = $(OBJECTS:.o=.d) build/main.d
-include $(DEPS)

build/%.o: src/%.c
	$(CC) -c $(CFLAGS) src/$*.c -o build/$*.o

build/%.d: src/%.c
	$(CC) -MM $(CFLAGS) src/$*.c > build/$*.d

libnoodle.so: $(OBJECTS) | $(DEPS)
	$(CC) $^ -shared $(CFLAGS) $(LFLAGS) -o $@

noodle: src/main.c libnoodle.so | $(DEPS)
	$(CC) $^ $(CFLAGS) $(LFLAGS) -o $@

.PHONY: format clean all
format:
	clang-format -i src/*.c src/*.h

clean:
	-rm -rf build $(TARGETS)

.PHONY: all
all: $(TARGETS)

.DEFAULT_GOAL = all
