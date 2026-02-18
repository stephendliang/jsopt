#pragma once

#include <stdint.h>
#include <string.h>

// Node: 16-byte flat representation of every token and AST node
// 4 nodes per cache line
typedef struct {
    uint8_t  kind;
    uint8_t  flags;
    uint16_t op;
    uint32_t start;
    uint32_t data[2];
} Node;

_Static_assert(sizeof(Node) == 16, "Node must be 16 bytes");

// NodeKind: explicit, stable enum values
// Tokens 0-127, AST compounds 128-255
typedef enum {
    // Leaves: persist as AST nodes (0-15)
    NODE_IDENT = 0,
    NODE_NUMBER, NODE_STRING, NODE_REGEX,
    NODE_TEMPLATE_FULL, NODE_TEMPLATE_HEAD, NODE_TEMPLATE_MID, NODE_TEMPLATE_TAIL,
    NODE_TRUE, NODE_FALSE, NODE_NULL, NODE_THIS, NODE_SUPER,
    // 13-15 reserved

    // Keywords (16-55)
    NODE_KW_ASYNC = 16, NODE_KW_AWAIT, NODE_KW_BREAK, NODE_KW_CASE,
    NODE_KW_CATCH, NODE_KW_CLASS, NODE_KW_CONST, NODE_KW_CONTINUE,
    NODE_KW_DEBUGGER, NODE_KW_DEFAULT, NODE_KW_DELETE, NODE_KW_DO,
    NODE_KW_ELSE, NODE_KW_EXPORT, NODE_KW_EXTENDS, NODE_KW_FINALLY,
    NODE_KW_FOR, NODE_KW_FUNCTION, NODE_KW_IF, NODE_KW_IMPORT,
    NODE_KW_IN, NODE_KW_INSTANCEOF, NODE_KW_LET, NODE_KW_NEW,
    NODE_KW_RETURN, NODE_KW_STATIC, NODE_KW_SWITCH, NODE_KW_THROW,
    NODE_KW_TRY, NODE_KW_TYPEOF, NODE_KW_VAR, NODE_KW_VOID,
    NODE_KW_WHILE, NODE_KW_WITH, NODE_KW_YIELD,
    // 51-55 reserved

    // Punctuation (56-71): consumed by parser, become dead
    NODE_LBRACE = 56, NODE_RBRACE, NODE_LPAREN, NODE_RPAREN,
    NODE_LBRACKET, NODE_RBRACKET, NODE_SEMI, NODE_COMMA,
    NODE_COLON, NODE_DOT, NODE_DOT_DOT_DOT, NODE_QUESTION,
    NODE_QUESTION_DOT, NODE_QUESTION_QUESTION, NODE_ARROW_TOK,

    // Operators (72-126): stored in .op field of compounds
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
    // 114-126 reserved

    NODE_EOF = 127,

    // AST compounds (128-255): produced by parser
    NODE_BINARY = 128, NODE_UNARY, NODE_UPDATE, NODE_ASSIGN,
    NODE_TERNARY, NODE_CALL, NODE_NEW, NODE_MEMBER, NODE_INDEX,
    NODE_ARRAY, NODE_OBJECT, NODE_FUNC_EXPR, NODE_ARROW,
    NODE_SEQUENCE, NODE_SPREAD, NODE_YIELD, NODE_AWAIT, NODE_TEMPLATE,

    NODE_BLOCK, NODE_EMPTY, NODE_EXPR_STMT,
    NODE_IF, NODE_WHILE, NODE_DO_WHILE,
    NODE_FOR, NODE_FOR_IN, NODE_FOR_OF,
    NODE_SWITCH, NODE_CASE,
    NODE_BREAK, NODE_CONTINUE, NODE_RETURN, NODE_THROW,
    NODE_TRY, NODE_CATCH, NODE_DEBUGGER, NODE_WITH, NODE_LABELED,

    NODE_VAR_DECL, NODE_DECLARATOR, NODE_FUNC_DECL,
    NODE_CLASS, NODE_CLASS_BODY, NODE_METHOD, NODE_PROPERTY,

    NODE_ARRAY_PATTERN, NODE_OBJECT_PATTERN, NODE_REST, NODE_ASSIGN_PATTERN,

    NODE_IMPORT, NODE_EXPORT, NODE_IMPORT_SPEC, NODE_EXPORT_SPEC,

    NODE_PROGRAM,

    NODE_COUNT
} NodeKind;

// Classification macros
#define IS_LEAF(k)      ((k) < 16)
#define IS_KEYWORD(k)   ((k) >= 16 && (k) < 56)
#define IS_PUNCT(k)     ((k) >= 56 && (k) < 72)
#define IS_OPERATOR(k)  ((k) >= 72 && (k) < 127)
#define IS_TOKEN(k)     ((k) <= 127)
#define IS_COMPOUND(k)  ((k) > 127)

// Node flag constants
#define NODE_FLAG_ASYNC     (1 << 0)
#define NODE_FLAG_GENERATOR (1 << 1)
#define NODE_FLAG_CONST     (1 << 2)
#define NODE_FLAG_LET       (1 << 3)
#define NODE_FLAG_STATIC    (1 << 4)
#define NODE_FLAG_COMPUTED  (1 << 5)
#define NODE_FLAG_SHORTHAND (1 << 6)
#define NODE_FLAG_METHOD    (1 << 7)

// Node access macros
#define NODE_NULL_IDX     0
#define NODE_LEN_OVERFLOW 0xFFFF

#define NODE(arr, idx)      (&(arr)->nodes[(idx)])
#define NODE_KIND(arr, idx) ((arr)->nodes[(idx)].kind)
#define NODE_VALID(idx)     ((idx) != 0)

// Only valid for token nodes where IS_TOKEN(n->kind)
#define NODE_END(n)  (((n)->op == NODE_LEN_OVERFLOW) ? (n)->data[1] : (n)->start + (n)->op)
#define NODE_LEN(n)  (NODE_END(n) - (n)->start)
#define TOKEN_END(n) (((n)->op == NODE_LEN_OVERFLOW) ? (n)->data[1] : (n)->start + (n)->op)

// Arena limit: 16M nodes = 256 MB virtual reservation
#define NODE_MAX_NODES (1 << 24)

// NodeArray
typedef struct {
    Node    *nodes;
    uint32_t count;
    uint32_t capacity;
    uint32_t token_end;
    uint32_t root;
} NodeArray;

// NodeArray API
int      node_array_init(NodeArray *arr, uint32_t capacity);
void     node_array_free(NodeArray *arr);
uint32_t node_push_token(NodeArray *arr, NodeKind kind,
                         uint32_t start, uint32_t len, uint32_t line);
uint32_t node_push(NodeArray *arr, NodeKind kind, uint8_t flags,
                   uint16_t op, uint32_t start,
                   uint32_t d0, uint32_t d1);
// Reserve count consecutive slots. Returns index of first.
// Caller accounts for subsequent node_push (make_list reserves count+1).
uint32_t node_reserve(NodeArray *arr, uint32_t count);

// EMIT: lexer hot path for token emission
// lex must have .nodes (NodeArray) and .line (uint32_t)
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
