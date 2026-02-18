CC      = gcc
CFLAGS  = -std=c11 -O3 -march=native -mavx512f -mavx512bw \
          -Wall -Wextra -Wpedantic -Werror \
          -Iinclude
LDFLAGS =

# Debug build: make DEBUG=1
ifdef DEBUG
CFLAGS  = -std=c11 -O0 -g -fsanitize=address,undefined -DDEBUG \
          -Wall -Wextra -Wpedantic -Werror \
          -Iinclude
LDFLAGS = -fsanitize=address,undefined
endif

BUILDDIR = build

all: $(BUILDDIR)/libnode.a $(BUILDDIR)/test_node

$(BUILDDIR)/node.o: src/node.c include/jsopt/node.h
	@mkdir -p $(BUILDDIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILDDIR)/libnode.a: $(BUILDDIR)/node.o
	ar rcs $@ $^

$(BUILDDIR)/test_node.o: tests/test_node.c include/jsopt/node.h
	@mkdir -p $(BUILDDIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILDDIR)/test_node: $(BUILDDIR)/test_node.o $(BUILDDIR)/libnode.a
	$(CC) $(LDFLAGS) $^ -o $@

test: $(BUILDDIR)/test_node
	./$(BUILDDIR)/test_node

clean:
	rm -rf $(BUILDDIR)

.PHONY: all test clean
