#include "jsopt/node.h"
#include <stdio.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

static int tests_run = 0;
static int tests_failed = 0;

#define ASSERT(cond, msg) do { \
    tests_run++; \
    if (!(cond)) { \
        fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, msg); \
        tests_failed++; \
    } \
} while(0)

// struct layout
static void test_struct_layout(void) {
    ASSERT(sizeof(Node) == 16, "sizeof(Node) == 16");
    ASSERT(offsetof(Node, kind) == 0, "offsetof kind == 0");
    ASSERT(offsetof(Node, flags) == 1, "offsetof flags == 1");
    ASSERT(offsetof(Node, op) == 2, "offsetof op == 2");
    ASSERT(offsetof(Node, start) == 4, "offsetof start == 4");
    ASSERT(offsetof(Node, data) == 8, "offsetof data == 8");
}

// enum values and classification macros
static void test_enum_values(void) {
    ASSERT(NODE_IDENT == 0, "NODE_IDENT == 0");
    ASSERT(NODE_EOF == 127, "NODE_EOF == 127");
    ASSERT(NODE_BINARY == 128, "NODE_BINARY == 128");

    // leaf boundaries
    ASSERT(IS_LEAF(0), "IS_LEAF(0)");
    ASSERT(IS_LEAF(15), "IS_LEAF(15)");
    ASSERT(!IS_LEAF(16), "!IS_LEAF(16)");

    // keyword boundaries
    ASSERT(IS_KEYWORD(NODE_KW_ASYNC), "IS_KEYWORD(NODE_KW_ASYNC)");
    ASSERT(IS_KEYWORD(NODE_KW_YIELD), "IS_KEYWORD(NODE_KW_YIELD)");
    ASSERT(IS_KEYWORD(55), "IS_KEYWORD(55)");
    ASSERT(!IS_KEYWORD(56), "!IS_KEYWORD(56)");

    // punctuation boundaries
    ASSERT(IS_PUNCT(NODE_LBRACE), "IS_PUNCT(NODE_LBRACE)");
    ASSERT(IS_PUNCT(NODE_ARROW_TOK), "IS_PUNCT(NODE_ARROW_TOK)");
    ASSERT(!IS_PUNCT(72), "!IS_PUNCT(72)");

    // operator boundaries
    ASSERT(IS_OPERATOR(NODE_PLUS), "IS_OPERATOR(NODE_PLUS)");
    ASSERT(IS_OPERATOR(126), "IS_OPERATOR(126)");
    ASSERT(!IS_OPERATOR(127), "!IS_OPERATOR(127)");

    // token/compound boundary
    ASSERT(IS_TOKEN(127), "IS_TOKEN(127)");
    ASSERT(!IS_TOKEN(128), "!IS_TOKEN(128)");
    ASSERT(IS_COMPOUND(128), "IS_COMPOUND(128)");
    ASSERT(!IS_COMPOUND(127), "!IS_COMPOUND(127)");
}

// flag constants
static void test_flags(void) {
    ASSERT(NODE_FLAG_ASYNC == 1, "NODE_FLAG_ASYNC == 1");
    ASSERT(NODE_FLAG_GENERATOR == 2, "NODE_FLAG_GENERATOR == 2");
    ASSERT(NODE_FLAG_CONST == 4, "NODE_FLAG_CONST == 4");
    ASSERT(NODE_FLAG_LET == 8, "NODE_FLAG_LET == 8");
    ASSERT(NODE_FLAG_STATIC == 16, "NODE_FLAG_STATIC == 16");
    ASSERT(NODE_FLAG_COMPUTED == 32, "NODE_FLAG_COMPUTED == 32");
    ASSERT(NODE_FLAG_SHORTHAND == 64, "NODE_FLAG_SHORTHAND == 64");
    ASSERT(NODE_FLAG_METHOD == 128, "NODE_FLAG_METHOD == 128");

    uint8_t all = NODE_FLAG_ASYNC | NODE_FLAG_GENERATOR | NODE_FLAG_CONST |
                  NODE_FLAG_LET | NODE_FLAG_STATIC | NODE_FLAG_COMPUTED |
                  NODE_FLAG_SHORTHAND | NODE_FLAG_METHOD;
    ASSERT(all == 0xFF, "all flags OR'd == 0xFF");
}

// array init
static void test_array_init(void) {
    NodeArray arr;
    int rc = node_array_init(&arr, 64);
    ASSERT(rc == 0, "init returns 0");
    ASSERT(arr.count == 1, "count == 1 after init");
    ASSERT(arr.capacity == NODE_MAX_NODES, "capacity == NODE_MAX_NODES");
    ASSERT(arr.nodes != NULL, "nodes != NULL");
    ASSERT(((uintptr_t)arr.nodes % 64) == 0, "page-aligned (>= 64)");
    ASSERT(arr.nodes[0].kind == 0, "sentinel kind == 0");
    ASSERT(arr.nodes[0].flags == 0, "sentinel flags == 0");
    ASSERT(arr.nodes[0].op == 0, "sentinel op == 0");
    ASSERT(arr.nodes[0].start == 0, "sentinel start == 0");
    ASSERT(arr.token_end == 0, "token_end == 0");
    ASSERT(arr.root == 0, "root == 0");
    node_array_free(&arr);
}

// push many nodes (arena handles it without growth)
static void test_push_past_initial(void) {
    NodeArray arr;
    node_array_init(&arr, 4);

    // push well past what old initial capacity would have been
    for (uint32_t i = 0; i < 1000; i++) {
        uint32_t idx = node_push_token(&arr, NODE_IDENT, i * 10, 3, i + 1);
        ASSERT(idx == i + 1, "push_past_initial: index correct");
    }
    ASSERT(arr.count == 1001, "push_past_initial: count == 1001");

    // verify data integrity
    for (uint32_t i = 0; i < 1000; i++) {
        Node *n = &arr.nodes[i + 1];
        ASSERT(n->kind == NODE_IDENT, "push_past_initial: kind correct");
        ASSERT(n->start == i * 10, "push_past_initial: start correct");
        ASSERT(n->data[0] == i + 1, "push_past_initial: line correct");
    }

    node_array_free(&arr);
}

// push_token normal
static void test_push_token_normal(void) {
    NodeArray arr;
    node_array_init(&arr, 64);

    uint32_t idx = node_push_token(&arr, NODE_IDENT, 10, 5, 1);
    ASSERT(idx == 1, "first push returns 1");
    ASSERT(arr.count == 2, "count == 2");

    Node *n = &arr.nodes[idx];
    ASSERT(n->kind == NODE_IDENT, "kind == NODE_IDENT");
    ASSERT(n->flags == 0, "flags == 0");
    ASSERT(n->op == 5, "op == 5");
    ASSERT(n->start == 10, "start == 10");
    ASSERT(n->data[0] == 1, "data[0] == 1 (line)");
    ASSERT(n->data[1] == 0, "data[1] == 0 (no overflow)");
    ASSERT(TOKEN_END(n) == 15, "TOKEN_END == 15");
    ASSERT(NODE_END(n) == 15, "NODE_END == 15");

    node_array_free(&arr);
}

// push_token overflow
static void test_push_token_overflow(void) {
    NodeArray arr;
    node_array_init(&arr, 64);

    uint32_t idx = node_push_token(&arr, NODE_STRING, 100, 70000, 5);
    Node *n = &arr.nodes[idx];
    ASSERT(n->op == NODE_LEN_OVERFLOW, "op == NODE_LEN_OVERFLOW");
    ASSERT(n->data[1] == 100 + 70000, "data[1] == start + 70000");
    ASSERT(TOKEN_END(n) == 100 + 70000, "TOKEN_END overflow correct");
    ASSERT(NODE_END(n) == 100 + 70000, "NODE_END overflow correct");

    node_array_free(&arr);
}

// push_token sequential
static void test_push_token_sequential(void) {
    NodeArray arr;
    node_array_init(&arr, 64);

    for (uint32_t i = 0; i < 10; i++) {
        uint32_t idx = node_push_token(&arr, NODE_IDENT, i * 10, 3, i + 1);
        ASSERT(idx == i + 1, "sequential push returns i+1");
    }
    ASSERT(arr.count == 11, "count == 11 after 10 pushes");

    for (uint32_t i = 0; i < 10; i++) {
        Node *n = &arr.nodes[i + 1];
        ASSERT(n->kind == NODE_IDENT, "seq node kind correct");
        ASSERT(n->start == i * 10, "seq node start correct");
        ASSERT(n->data[0] == i + 1, "seq node line correct");
    }

    node_array_free(&arr);
}

// push compound
static void test_push_compound(void) {
    NodeArray arr;
    node_array_init(&arr, 64);

    node_push_token(&arr, NODE_NUMBER, 0, 1, 1);
    node_push_token(&arr, NODE_NUMBER, 4, 1, 1);

    uint32_t idx = node_push(&arr, NODE_BINARY, 0, NODE_PLUS, 5, 1, 2);
    Node *n = &arr.nodes[idx];
    ASSERT(n->kind == NODE_BINARY, "compound kind == NODE_BINARY");
    ASSERT(n->op == NODE_PLUS, "compound op == NODE_PLUS");
    ASSERT(n->start == 5, "compound start == 5");
    ASSERT(n->data[0] == 1, "compound data[0] == 1");
    ASSERT(n->data[1] == 2, "compound data[1] == 2");

    node_array_free(&arr);
}

// push flags
static void test_push_flags(void) {
    NodeArray arr;
    node_array_init(&arr, 64);

    uint8_t f = NODE_FLAG_ASYNC | NODE_FLAG_GENERATOR;
    uint32_t idx = node_push(&arr, NODE_FUNC_DECL, f, 0, 0, 3, 4);
    Node *n = &arr.nodes[idx];
    ASSERT(n->flags == f, "flags == ASYNC|GENERATOR");

    node_array_free(&arr);
}

// reserve
static void test_reserve(void) {
    NodeArray arr;
    node_array_init(&arr, 64);

    uint32_t before = arr.count;
    uint32_t first = node_reserve(&arr, 5);
    ASSERT(first == before, "reserve returns previous count");
    ASSERT(arr.count == before + 5, "count advanced by 5");

    // reserved slots are zeroed
    for (uint32_t i = 0; i < 5; i++) {
        Node *n = &arr.nodes[first + i];
        ASSERT(n->kind == 0, "reserved slot zeroed (kind)");
        ASSERT(n->flags == 0, "reserved slot zeroed (flags)");
        ASSERT(n->op == 0, "reserved slot zeroed (op)");
        ASSERT(n->start == 0, "reserved slot zeroed (start)");
    }

    // subsequent push goes after reserved block
    uint32_t idx = node_push_token(&arr, NODE_IDENT, 0, 1, 1);
    ASSERT(idx == first + 5, "push after reserve goes to correct slot");

    node_array_free(&arr);
}

// reserve large block
static void test_reserve_large(void) {
    NodeArray arr;
    node_array_init(&arr, 4);

    uint32_t first = node_reserve(&arr, 10);
    ASSERT(first == 1, "reserve starts at 1");
    ASSERT(arr.count == 11, "count advanced by 10");

    // reserved slots zeroed (mmap guarantees this)
    for (uint32_t i = 0; i < 10; i++) {
        ASSERT(arr.nodes[first + i].kind == 0, "reserve_large slot zeroed");
    }

    // push after reserve goes to correct slot
    uint32_t idx = node_push_token(&arr, NODE_IDENT, 0, 1, 1);
    ASSERT(idx == 11, "push after large reserve correct");

    node_array_free(&arr);
}

// double free safety: munmap(NULL) is skipped
static void test_double_free(void) {
    NodeArray arr;
    node_array_init(&arr, 64);
    node_array_free(&arr);
    ASSERT(arr.nodes == NULL, "nodes == NULL after free");
    // second free should not crash (nodes is NULL, skipped)
    node_array_free(&arr);
    ASSERT(arr.nodes == NULL, "double free safe");
}

// free
static void test_free(void) {
    NodeArray arr;
    node_array_init(&arr, 64);
    node_array_free(&arr);
    ASSERT(arr.nodes == NULL, "nodes == NULL after free");
    ASSERT(arr.count == 0, "count == 0 after free");
    ASSERT(arr.capacity == 0, "capacity == 0 after free");
}

// NODE_NULL_IDX
static void test_null_idx(void) {
    ASSERT(NODE_NULL_IDX == 0, "NODE_NULL_IDX == 0");
    ASSERT(!NODE_VALID(0), "NODE_VALID(0) is false");
    ASSERT(NODE_VALID(1), "NODE_VALID(1) is true");
}

// NODE_KIND macro
static void test_node_kind_macro(void) {
    NodeArray arr;
    node_array_init(&arr, 64);
    node_push_token(&arr, NODE_STRING, 0, 5, 1);
    ASSERT(NODE_KIND(&arr, 1) == NODE_STRING, "NODE_KIND macro works");
    node_array_free(&arr);
}

// NODE macro
static void test_node_macro(void) {
    NodeArray arr;
    node_array_init(&arr, 64);
    node_push_token(&arr, NODE_IDENT, 0, 3, 1);

    Node *n = NODE(&arr, 1);
    ASSERT(n == &arr.nodes[1], "NODE returns correct pointer");

    n->flags = NODE_FLAG_CONST;
    ASSERT(arr.nodes[1].flags == NODE_FLAG_CONST, "modification through NODE visible");

    node_array_free(&arr);
}

int main(void) {
    test_struct_layout();
    test_enum_values();
    test_flags();
    test_array_init();
    test_push_past_initial();
    test_push_token_normal();
    test_push_token_overflow();
    test_push_token_sequential();
    test_push_compound();
    test_push_flags();
    test_reserve();
    test_reserve_large();
    test_double_free();
    test_free();
    test_null_idx();
    test_node_kind_macro();
    test_node_macro();

    printf("%d tests, %d failed\n", tests_run, tests_failed);
    return tests_failed ? 1 : 0;
}
