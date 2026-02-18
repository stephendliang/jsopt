# Phase 1: Foundation — Node + NodeArray + Build

> Scope: `node.h`, `node.c`, build system, C test harness.
> Nothing else. No lexer, no parser, no I/O.

---

## Deliverables

| File | Purpose |
|------|---------|
| `include/jsopt/node.h` | Node struct, NodeKind enum, NodeArray struct, all macros |
| `src/node.c` | NodeArray init/free/push/reserve, aligned allocation |
| `tests/test_node.c` | Unit tests for everything above |
| `Makefile` | Build system (`-O3 -march=native -mavx512f -mavx512bw`) |

---

## 1. `node.h` — What It Must Contain

### 1.1 Node Struct

```c
typedef struct {
    uint8_t  kind;
    uint8_t  flags;
    uint16_t op;
    uint32_t start;
    uint32_t data[2];
} Node;
```

**Compile-time assertion:** `_Static_assert(sizeof(Node) == 16, "Node must be 16 bytes")`. This is the single most important invariant in the project. If it breaks, everything downstream is wrong.

### 1.2 NodeKind Enum

Every value is explicit and stable. Include `NODE_COUNT` as the final entry.

```c
typedef enum {
    /* ================================================================
     * TOKENS (0-127): Produced by lexer
     * ================================================================ */

    /*--- Leaves: persist as AST nodes (0-15) ---*/
    NODE_IDENT = 0,
    NODE_NUMBER, NODE_STRING, NODE_REGEX,
    NODE_TEMPLATE_FULL, NODE_TEMPLATE_HEAD, NODE_TEMPLATE_MID, NODE_TEMPLATE_TAIL,
    NODE_TRUE, NODE_FALSE, NODE_NULL, NODE_THIS, NODE_SUPER,
    /* 13-15 reserved */

    /*--- Keywords (16-55) ---*/
    NODE_KW_ASYNC = 16, NODE_KW_AWAIT, NODE_KW_BREAK, NODE_KW_CASE,
    NODE_KW_CATCH, NODE_KW_CLASS, NODE_KW_CONST, NODE_KW_CONTINUE,
    NODE_KW_DEBUGGER, NODE_KW_DEFAULT, NODE_KW_DELETE, NODE_KW_DO,
    NODE_KW_ELSE, NODE_KW_EXPORT, NODE_KW_EXTENDS, NODE_KW_FINALLY,
    NODE_KW_FOR, NODE_KW_FUNCTION, NODE_KW_IF, NODE_KW_IMPORT,
    NODE_KW_IN, NODE_KW_INSTANCEOF, NODE_KW_LET, NODE_KW_NEW,
    NODE_KW_RETURN, NODE_KW_STATIC, NODE_KW_SWITCH, NODE_KW_THROW,
    NODE_KW_TRY, NODE_KW_TYPEOF, NODE_KW_VAR, NODE_KW_VOID,
    NODE_KW_WHILE, NODE_KW_WITH, NODE_KW_YIELD,
    /* 51-55 reserved */

    /*--- Punctuation (56-71): consumed by parser, become dead ---*/
    NODE_LBRACE = 56, NODE_RBRACE, NODE_LPAREN, NODE_RPAREN,
    NODE_LBRACKET, NODE_RBRACKET, NODE_SEMI, NODE_COMMA,
    NODE_COLON, NODE_DOT, NODE_DOT_DOT_DOT, NODE_QUESTION,
    NODE_QUESTION_DOT, NODE_QUESTION_QUESTION, NODE_ARROW_TOK,

    /*--- Operators (72-126): stored in .op field of compounds ---*/
    NODE_PLUS = 72, NODE_MINUS, NODE_STAR, NODE_SLASH,
    NODE_PERCENT, NODE_STAR_STAR, NODE_PLUS_PLUS, NODE_MINUS_MINUS,
    NODE_LT, NODE_GT, NODE_LT_EQ, NODE_GT_EQ,
    NODE_EQ_EQ, NODE_EQ_EQ_EQ, NODE_BANG_EQ, NODE_BANG_EQ_EQ,
    NODE_LT_LT, NODE_GT_GT, NODE_GT_GT_GT,
    NODE_AMP, NODE_PIPE, NODE_CARET, NODE_TILDE, NODE_BANG,
    NODE_AMP_AMP, NODE_PIPE_PIPE,
    NODE_EQ, NODE_PLUS_EQ, NODE_MINUS_EQ, NODE_STAR_EQ,
    NODE_SLASH_EQ, NODE_PERCENT_EQ, NODE_STAR_STAR_EQ,
    NODE_LT_LT_EQ, NODE_GT_GT_EQ, NODE_GT_GT_GT_EQ,
    NODE_AMP_EQ, NODE_PIPE_EQ, NODE_CARET_EQ,
    NODE_AMP_AMP_EQ, NODE_PIPE_PIPE_EQ, NODE_QUESTION_QUESTION_EQ,
    /* 114-126 reserved */

    NODE_EOF = 127,

    /* ================================================================
     * AST COMPOUNDS (128-255): Produced by parser
     * ================================================================ */

    /*--- Expressions ---*/
    NODE_BINARY = 128, NODE_UNARY, NODE_UPDATE, NODE_ASSIGN,
    NODE_TERNARY, NODE_CALL, NODE_NEW, NODE_MEMBER, NODE_INDEX,
    NODE_ARRAY, NODE_OBJECT, NODE_FUNC_EXPR, NODE_ARROW,
    NODE_SEQUENCE, NODE_SPREAD, NODE_YIELD, NODE_AWAIT, NODE_TEMPLATE,

    /*--- Statements ---*/
    NODE_BLOCK, NODE_EMPTY, NODE_EXPR_STMT,
    NODE_IF, NODE_WHILE, NODE_DO_WHILE,
    NODE_FOR, NODE_FOR_IN, NODE_FOR_OF,
    NODE_SWITCH, NODE_CASE,
    NODE_BREAK, NODE_CONTINUE, NODE_RETURN, NODE_THROW,
    NODE_TRY, NODE_CATCH, NODE_DEBUGGER, NODE_WITH, NODE_LABELED,

    /*--- Declarations ---*/
    NODE_VAR_DECL, NODE_DECLARATOR, NODE_FUNC_DECL,
    NODE_CLASS, NODE_CLASS_BODY, NODE_METHOD, NODE_PROPERTY,

    /*--- Patterns ---*/
    NODE_ARRAY_PATTERN, NODE_OBJECT_PATTERN, NODE_REST, NODE_ASSIGN_PATTERN,

    /*--- Module ---*/
    NODE_IMPORT, NODE_EXPORT, NODE_IMPORT_SPEC, NODE_EXPORT_SPEC,

    /*--- Root ---*/
    NODE_PROGRAM,

    NODE_COUNT
} NodeKind;
```

Note: `NODE_ARROW_TOK` (punctuation, the `=>` token) is distinct from `NODE_ARROW` (AST compound, the arrow function expression node). The `_TOK` suffix disambiguates.

### 1.3 Classification Macros

```c
#define IS_LEAF(k)      ((k) < 16)
#define IS_KEYWORD(k)   ((k) >= 16 && (k) < 56)
#define IS_PUNCT(k)     ((k) >= 56 && (k) < 72)
#define IS_OPERATOR(k)  ((k) >= 72 && (k) < 127)
#define IS_TOKEN(k)     ((k) <= 127)
#define IS_COMPOUND(k)  ((k) > 127)
```

### 1.4 Node Flag Constants

```c
#define NODE_FLAG_ASYNC     (1 << 0)
#define NODE_FLAG_GENERATOR (1 << 1)
#define NODE_FLAG_CONST     (1 << 2)
#define NODE_FLAG_LET       (1 << 3)
#define NODE_FLAG_STATIC    (1 << 4)
#define NODE_FLAG_COMPUTED  (1 << 5)
#define NODE_FLAG_SHORTHAND (1 << 6)
#define NODE_FLAG_METHOD    (1 << 7)
```

### 1.5 Node Access Macros

```c
#define NODE_NULL_IDX     0
#define NODE_LEN_OVERFLOW 0xFFFF

#define NODE(arr, idx)      (&(arr)->nodes[(idx)])
#define NODE_KIND(arr, idx) ((arr)->nodes[(idx)].kind)
#define NODE_VALID(idx)     ((idx) != 0)

#define NODE_END(n)  (((n)->op == NODE_LEN_OVERFLOW) ? (n)->data[1] : (n)->start + (n)->op)
#define NODE_LEN(n)  (NODE_END(n) - (n)->start)
```

Note: `NODE_END` and `NODE_LEN` are only valid for token nodes (where `IS_TOKEN(n->kind)`). For compounds, `op` stores the operator kind. This is a caller invariant, not runtime-checked.

### 1.6 NodeArray Struct

```c
typedef struct {
    Node    *nodes;
    uint32_t count;
    uint32_t capacity;
    uint32_t token_end;
    uint32_t root;
} NodeArray;
```

### 1.7 NodeArray API Declarations

```c
int      node_array_init(NodeArray *arr, uint32_t capacity);
void     node_array_free(NodeArray *arr);
uint32_t node_push_token(NodeArray *arr, NodeKind kind,
                         uint32_t start, uint32_t len, uint32_t line);
uint32_t node_push(NodeArray *arr, NodeKind kind, uint8_t flags,
                   uint16_t op, uint32_t start,
                   uint32_t d0, uint32_t d1);
uint32_t node_reserve(NodeArray *arr, uint32_t count);
```

### 1.8 EMIT Macro

```c
#define EMIT(lex, k, s, e) do { \
    NodeArray *_a = &(lex)->nodes; \
    uint32_t _len = (e) - (s); \
    if (__builtin_expect(_a->count < _a->capacity, 1)) { \
        Node *_n = &_a->nodes[_a->count++]; \
        _n->kind = (k); _n->flags = 0; \
        _n->op = (_len <= 0xFFFE) ? (uint16_t)_len : NODE_LEN_OVERFLOW; \
        _n->start = (s); \
        _n->data[0] = (lex)->line; \
        _n->data[1] = (_len > 0xFFFE) ? (e) : 0; \
    } else { \
        node_push_token(_a, (k), (s), _len, (lex)->line); \
    } \
} while(0)
```

This lives in `node.h` even though it references `(lex)->line` and `(lex)->nodes` — it's a lexer-hot-path macro that the lexer will use directly. The `lex` parameter is any struct whose first relevant fields include `.nodes` (NodeArray) and `.line` (uint32_t). This is documented, not enforced by the type system.

### 1.9 `token_end` Inline

```c
static inline uint32_t token_end(const Node *n) {
    return (n->op == NODE_LEN_OVERFLOW) ? n->data[1] : n->start + n->op;
}
```

---

## 2. `node.c` — What It Must Implement

### 2.1 `node_array_init`

```
int node_array_init(NodeArray *arr, uint32_t capacity)
```

- Allocate `capacity * sizeof(Node)` bytes, **64-byte aligned** (`aligned_alloc(64, ...)`).
- The capacity must be rounded up to a multiple of 4 (so the allocation is a multiple of 64 bytes, i.e. whole cache lines).
- Zero the entire allocation (`memset`).
- Set `arr->count = 1` — index 0 is the null sentinel. `nodes[0]` is already zero from the memset, which makes it `{kind=NODE_IDENT(0), flags=0, op=0, start=0, data={0,0}}`. This is fine; nothing should dereference index 0.
- Set `arr->token_end = 0`, `arr->root = 0`.
- Return 0 on success, nonzero on allocation failure.

Edge case: if `capacity < 4`, clamp to 4.

### 2.2 `node_array_free`

```
void node_array_free(NodeArray *arr)
```

- `free(arr->nodes)` (aligned_alloc memory is freed with `free` on POSIX).
- Zero the struct.

### 2.3 `node_push_token`

```
uint32_t node_push_token(NodeArray *arr, NodeKind kind,
                         uint32_t start, uint32_t len, uint32_t line)
```

- If `arr->count >= arr->capacity`, grow (see 2.5).
- Fill `nodes[count]`:
  - `kind = kind`, `flags = 0`
  - `op = (len <= 0xFFFE) ? (uint16_t)len : NODE_LEN_OVERFLOW`
  - `start = start`
  - `data[0] = line`
  - `data[1] = (len > 0xFFFE) ? (start + len) : 0`
- Increment `count`, return the index of the new node.

### 2.4 `node_push`

```
uint32_t node_push(NodeArray *arr, NodeKind kind, uint8_t flags,
                   uint16_t op, uint32_t start,
                   uint32_t d0, uint32_t d1)
```

- If `arr->count >= arr->capacity`, grow.
- Fill `nodes[count]`: `kind`, `flags`, `op`, `start`, `data[0]=d0`, `data[1]=d1`.
- Increment `count`, return the index.

### 2.5 Growth Strategy

When the array is full:
- New capacity = `old_capacity * 2` (double).
- Allocate new 64-byte-aligned buffer.
- `memcpy` existing nodes.
- `free` old buffer.
- If allocation fails, abort (this is a fatal OOM — the program cannot continue).

Do NOT use `realloc` — it does not guarantee 64-byte alignment.

### 2.6 `node_reserve`

```
uint32_t node_reserve(NodeArray *arr, uint32_t count)
```

- Ensure `arr->count + count <= arr->capacity`, growing if needed.
- Save `arr->count` as the return value (the index of the first reserved slot).
- Advance `arr->count += count`.
- Zero the reserved slots (`memset`).
- Return the saved index.

This is used by the parser's `make_list` to allocate consecutive child slots before the parent node. The caller writes into the reserved slots directly.

---

## 3. Build System

### Makefile

Compiler: `gcc` or `clang`. Flags:

```makefile
CC      = gcc
CFLAGS  = -std=c11 -O3 -march=native -mavx512f -mavx512bw \
          -Wall -Wextra -Wpedantic -Werror \
          -Iinclude
LDFLAGS =
```

For debug builds: `-O0 -g -fsanitize=address,undefined -DDEBUG`.

Targets:
- `all`: build `libnode.a` (just `node.o` for now) and `test_node`
- `test`: build and run `test_node`
- `clean`: remove build artifacts

Directory layout:
```
include/jsopt/node.h
src/node.c
tests/test_node.c
Makefile
```

Build artifacts go into `build/` (gitignored).

---

## 4. C Test Harness: `tests/test_node.c`

A self-contained test runner. No external frameworks. Pattern:

```c
static int tests_run = 0;
static int tests_failed = 0;

#define ASSERT(cond, msg) do { \
    tests_run++; \
    if (!(cond)) { \
        fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, msg); \
        tests_failed++; \
    } \
} while(0)
```

Exit 0 if all pass, 1 if any fail. Print summary at end.

### Tests To Write

**Struct layout (compile-time + runtime):**
- `sizeof(Node) == 16`
- `offsetof(Node, kind) == 0`
- `offsetof(Node, flags) == 1`
- `offsetof(Node, op) == 2`
- `offsetof(Node, start) == 4`
- `offsetof(Node, data) == 8`

**NodeKind enum values:**
- `NODE_IDENT == 0`
- `NODE_EOF == 127`
- `NODE_BINARY == 128`
- Range boundaries: `IS_LEAF(0)` true, `IS_LEAF(15)` true, `IS_LEAF(16)` false
- `IS_KEYWORD(NODE_KW_ASYNC)` true, `IS_KEYWORD(NODE_KW_YIELD)` true, `IS_KEYWORD(55)` true, `IS_KEYWORD(56)` false
- `IS_PUNCT(NODE_LBRACE)` true, `IS_PUNCT(NODE_ARROW)` true, `IS_PUNCT(72)` false
- `IS_OPERATOR(NODE_PLUS)` true, `IS_OPERATOR(126)` true, `IS_OPERATOR(127)` false
- `IS_TOKEN(127)` true, `IS_TOKEN(128)` false
- `IS_COMPOUND(128)` true, `IS_COMPOUND(127)` false

**Flag constants:**
- Each flag is a distinct power of 2
- OR-ing all 8 flags gives 0xFF
- Each bit position is correct: `NODE_FLAG_ASYNC == 1`, `NODE_FLAG_METHOD == 128`

**NodeArray init:**
- After init with capacity 64: `arr.count == 1`, `arr.capacity >= 64`
- `arr.nodes != NULL`
- `((uintptr_t)arr.nodes % 64) == 0` (64-byte aligned)
- Sentinel: `arr.nodes[0].kind == 0` (all zeros)
- `arr.token_end == 0`, `arr.root == 0`

**NodeArray init edge cases:**
- Capacity 0 or 1 clamps to 4
- Capacity not a multiple of 4 rounds up

**node_push_token — normal:**
- Push a token with kind=NODE_IDENT, start=10, len=5, line=1
- Returns index 1 (first slot after sentinel)
- Node at index 1: `kind==NODE_IDENT`, `op==5`, `start==10`, `data[0]==1`, `data[1]==0`
- `token_end(&nodes[1]) == 15`
- `NODE_END(&nodes[1]) == 15`
- `arr.count == 2`

**node_push_token — overflow:**
- Push a token with len=70000 (exceeds 0xFFFE)
- `op == NODE_LEN_OVERFLOW` (0xFFFF)
- `data[1] == start + 70000`
- `token_end()` returns `start + 70000`
- `NODE_END()` returns `start + 70000`

**node_push_token — sequential:**
- Push 10 tokens sequentially
- Each returns the expected index (1 through 10)
- `arr.count == 11`
- All nodes accessible and correct

**node_push — compound:**
- Push a compound: `node_push(arr, NODE_BINARY, 0, NODE_PLUS, 5, 1, 2)`
- Returns correct index
- `kind == NODE_BINARY`, `op == NODE_PLUS`, `start == 5`, `data[0] == 1`, `data[1] == 2`

**node_push — flags:**
- Push with flags: `node_push(arr, NODE_FUNC_DECL, NODE_FLAG_ASYNC | NODE_FLAG_GENERATOR, 0, 0, 3, 4)`
- `flags == (NODE_FLAG_ASYNC | NODE_FLAG_GENERATOR)`

**node_reserve:**
- Reserve 5 slots
- Returns index of first slot
- `arr.count` advanced by 5
- All 5 reserved slots are zeroed
- Subsequent `node_push` goes into the slot after the reserved block

**Growth:**
- Init with capacity 4 (minimum). After init: count=1 (sentinel), capacity=4.
- Push 3 tokens: count becomes 4 (slots 1, 2, 3 filled).
- Push a 4th token: triggers growth because count(4) >= capacity(4).
- After growth: capacity >= 8, all 4 previous nodes preserved and correct.
- Alignment still 64-byte after reallocation.

**node_reserve + growth:**
- Init with capacity 4
- Reserve 10 slots (forces growth)
- Capacity >= 11 (at least), alignment preserved
- Reserved slots are zeroed

**node_array_free:**
- After free: `nodes == NULL`, `count == 0`, `capacity == 0`
- Double-free safety: free a zeroed struct doesn't crash (free(NULL) is safe)

**NODE_NULL_IDX:**
- `NODE_NULL_IDX == 0`
- `NODE_VALID(0) == 0` (false)
- `NODE_VALID(1) == 1` (true)

**NODE_KIND macro:**
- After pushing a token at index 1 with kind NODE_STRING: `NODE_KIND(&arr, 1) == NODE_STRING`

**NODE macro:**
- `NODE(&arr, 1)` returns a pointer to `arr.nodes[1]`
- Modifying through the pointer is visible

---

## 5. Traps and Decisions

**`aligned_alloc` portability:** POSIX (Linux, macOS) guarantees `free()` works on `aligned_alloc` pointers. Windows would need `_aligned_malloc`/`_aligned_free`. For now, target Linux only. Add a `#ifdef _WIN32` path later if needed.

**Growth during `make_list`:** The parser calls `node_reserve(count + 1)` then copies children into the reserved slots, then calls `node_push` for the parent. Because `reserve` already advanced `count` and the `+1` accounts for the parent, the push goes into the last reserved slot. This is subtle — `node_push` must NOT trigger growth after a reserve that already allocated the space. The `+1` in `make_list` is critical. This invariant should be documented in a comment in `node_reserve`.

**Zero-init vs sentinel:** Index 0 is all-zeros. `NODE_IDENT==0` means the sentinel *looks like* a zero-length identifier at offset 0. This is fine because nothing should access index 0 through normal code paths. `NODE_VALID(idx)` guards this.

**`op` field ambiguity:** For tokens, `op` = length. For compounds, `op` = operator kind (which is itself a NodeKind value). The caller must know which interpretation applies. `IS_TOKEN(kind)` disambiguates. This is a design-by-convention tradeoff for keeping Node at 16 bytes.

**No thread safety:** NodeArray is single-threaded. No locks, no atomics.

---

## 6. Acceptance Criteria

Phase 1 is done when:

1. `make test` runs `test_node` and all assertions pass.
2. `make` with `-O3 -march=native -mavx512f -mavx512bw -Werror` produces zero warnings.
3. `make` with `-fsanitize=address,undefined` and running `test_node` produces zero sanitizer errors.
4. `sizeof(Node) == 16` verified at compile time.
5. NodeArray allocation is 64-byte aligned, verified at runtime.
6. Growth works correctly (push beyond initial capacity, reserve beyond capacity).
7. The overflow sentinel (`op == 0xFFFF`, true end in `data[1]`) round-trips correctly.

No performance benchmarks in Phase 1 — there's nothing to benchmark yet. Correctness only.
