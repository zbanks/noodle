TARGET = noodle

CC=gcc

CFLAGS += -std=c11 -D_DEFAULT_SOURCE
CFLAGS += -Wall -Wextra -Wconversion -Werror
CFLAGS += -ggdb3
CFLAGS += -O3
CFLAGS += -Isrc/
LFLAGS = 

$(shell mkdir -p build)
OBJECTS = $(patsubst src/%.c,build/%.o,$(wildcard src/*.c))
DEPS = $(OBJECTS:.o=.d)
-include $(DEPS)

build/%.o: src/%.c
	$(CC) -c $(CFLAGS) src/$*.c -o build/$*.o
	$(CC) -MM $(CFLAGS) src/$*.c > build/$*.d

$(TARGET): $(OBJECTS)
	$(CC) $^ $(CFLAGS) $(LFLAGS) -o $@

.PHONY: format clean all
format:
	clang-format -i src/*.c src/*.h

clean:
	-rm -rf build $(TARGET)

.PHONY: all
all: $(TARGET)

.DEFAULT_GOAL = all
