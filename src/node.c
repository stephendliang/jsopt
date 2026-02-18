#include "jsopt/node.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <sys/mman.h>

#ifndef MAP_HUGE_2MB
#define MAP_HUGE_2MB (21 << MAP_HUGE_SHIFT)
#endif

int node_array_init(NodeArray *arr, uint32_t capacity) {
    (void)capacity; // ignored, always reserve max
    size_t size = (size_t)NODE_MAX_NODES * sizeof(Node);
    // Try 2MB huge pages first, fall back to regular pages
    Node *buf = mmap(NULL, size, PROT_READ | PROT_WRITE,
                     MAP_PRIVATE | MAP_ANONYMOUS | MAP_HUGETLB | MAP_HUGE_2MB,
                     -1, 0);
    if (buf == MAP_FAILED)
        buf = mmap(NULL, size, PROT_READ | PROT_WRITE,
                   MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (buf == MAP_FAILED) return -1;
    // mmap zeroes memory, no memset needed
    arr->nodes     = buf;
    arr->count     = 1; // index 0 is null sentinel
    arr->capacity  = NODE_MAX_NODES;
    arr->token_end = 0;
    arr->root      = 0;
    return 0;
}

void node_array_free(NodeArray *arr) {
    if (arr->nodes)
        munmap(arr->nodes, (size_t)arr->capacity * sizeof(Node));
    memset(arr, 0, sizeof(*arr));
}

uint32_t node_push_token(NodeArray *arr, NodeKind kind,
                         uint32_t start, uint32_t len, uint32_t line) {
    if (arr->count >= arr->capacity) {
        fprintf(stderr, "jsopt: node limit exceeded (%u)\n", arr->capacity);
        abort();
    }

    uint32_t idx = arr->count++;
    Node *n   = &arr->nodes[idx];
    n->kind   = kind;
    n->flags  = 0;
    n->op     = (len <= 0xFFFE) ? (uint16_t)len : NODE_LEN_OVERFLOW;
    n->start  = start;
    n->data[0] = line;
    n->data[1] = (len > 0xFFFE) ? (start + len) : 0;
    return idx;
}

uint32_t node_push(NodeArray *arr, NodeKind kind, uint8_t flags,
                   uint16_t op, uint32_t start,
                   uint32_t d0, uint32_t d1) {
    if (arr->count >= arr->capacity) {
        fprintf(stderr, "jsopt: node limit exceeded (%u)\n", arr->capacity);
        abort();
    }

    uint32_t idx = arr->count++;
    Node *n   = &arr->nodes[idx];
    n->kind   = kind;
    n->flags  = flags;
    n->op     = op;
    n->start  = start;
    n->data[0] = d0;
    n->data[1] = d1;
    return idx;
}

uint32_t node_reserve(NodeArray *arr, uint32_t count) {
    uint32_t needed = arr->count + count;
    if (needed > arr->capacity) {
        fprintf(stderr, "jsopt: node limit exceeded (%u)\n", arr->capacity);
        abort();
    }

    uint32_t first = arr->count;
    arr->count += count;
    // mmap pages are already zeroed on first touch
    return first;
}
