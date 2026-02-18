//! Ground truth harness for jsopt development.
//! Uses oxc as reference implementation for lexer, parser, scope, codegen, and mangler.
//!
//! Usage: ground-truth <mode> <file.js>
//! Modes: lex, ast, minify, mangle, scope, all
//!
//! NOTE: oxc API changes between versions. If this doesn't compile on your
//! oxc version, compiler errors will point to the exact fixes needed.

#![allow(unused_imports)]

use std::{env, fmt, fs, process};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_codegen::{Codegen, CodegenOptions, CommentOptions, LegalComment};
use oxc_mangler::{Mangler, MangleOptions};
use oxc_parser::lexer::{Kind, Lexer};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType, Span};

thread_local! {
    static UNSUPPORTED_COUNT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Print an unsupported node placeholder and increment the counter.
fn pr_unsupported(d: usize, kind: &str, detail: &str, span: Span) {
    UNSUPPORTED_COUNT.with(|c| c.set(c.get() + 1));
    pr(d, kind, detail, span);
    eprintln!("warning: unsupported node {} at {}:{}", kind, span.start, span.end);
}

fn unsupported_count() -> u32 {
    UNSUPPORTED_COUNT.with(|c| c.get())
}

fn reset_unsupported() {
    UNSUPPORTED_COUNT.with(|c| c.set(0));
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        usage();
    }

    let mode = &args[1];
    let path = &args[2];

    let source = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("error: {path}: {e}");
        process::exit(1);
    });

    let source_type = SourceType::from_path(path).unwrap_or_default();

    // Lex-only mode: no parsing needed
    if mode == "lex" || mode == "tokens" {
        let allocator = Allocator::default();
        cmd_lex(&source, source_type, &allocator);
        return;
    }

    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, &source, source_type).parse();

    let has_errors = !ret.errors.is_empty();
    if has_errors {
        eprintln!("=== PARSE ERRORS ({}) ===", ret.errors.len());
        for e in &ret.errors {
            eprintln!("  {e}");
        }
        if mode != "all" {
            process::exit(1);
        }
    }

    reset_unsupported();

    match mode.as_str() {
        "ast" | "parse" => cmd_ast(&ret.program, &source),
        "minify" => cmd_minify(&ret.program, false),
        "mangle" => cmd_minify(&ret.program, true),
        "scope" => cmd_scope(&ret.program),
        "all" => {
            println!("=== SOURCE: {path} ({} bytes, {} lines) ===", source.len(), source.lines().count());
            println!();
            {
                let lex_alloc = Allocator::default();
                cmd_lex(&source, source_type, &lex_alloc);
            }
            println!();
            cmd_ast(&ret.program, &source);
            println!();
            println!("=== MINIFY ===");
            cmd_minify(&ret.program, false);
            println!();
            println!();
            println!("=== MANGLE ===");
            cmd_minify(&ret.program, true);
            println!();
            println!();
            cmd_scope(&ret.program);
        }
        _ => {
            eprintln!("unknown mode: {mode}");
            usage();
        }
    }

    let unsup = unsupported_count();
    if unsup > 0 {
        eprintln!("error: {} unsupported AST node(s) encountered", unsup);
        process::exit(2);
    }

    if has_errors {
        process::exit(1);
    }
}

fn usage() -> ! {
    eprintln!("Usage: ground-truth <mode> <file.js>");
    eprintln!();
    eprintln!("Modes:");
    eprintln!("  lex      Full token stream (all tokens including punctuation/operators)");
    eprintln!("  ast      AST tree dump (one node per line, diffable)");
    eprintln!("  minify   Minified output (no mangling)");
    eprintln!("  mangle   Minified + mangled output");
    eprintln!("  scope    Scope analysis (per-reference resolution)");
    eprintln!("  all      All of the above");
    eprintln!();
    eprintln!("AST node count: ground-truth ast <file> | wc -l");
    process::exit(1);
}

// ============================================================================
// HELPERS
// ============================================================================

/// Escape a string so it fits on a single output line.
/// Replaces \n, \r, \t, \0, and other control chars with visible escape sequences.
fn escape_one_line(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => {
                out.push_str(&format!("\\u{{{:04x}}}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

fn pr(d: usize, kind: &str, detail: &str, span: Span) {
    if detail.is_empty() {
        println!("{:w$}{} {}:{}", "", kind, span.start, span.end, w = d * 2);
    } else {
        let safe = escape_one_line(detail);
        println!("{:w$}{} {} {}:{}", "", kind, safe, span.start, span.end, w = d * 2);
    }
}

fn snip(src: &str, span: Span) -> String {
    let s = span.start as usize;
    let e = (span.end as usize).min(src.len());
    if e <= s { return String::new(); }
    let text = &src[s..e];
    let text = if text.len() <= 50 {
        text
    } else {
        // Walk back from byte 50 to find a valid UTF-8 char boundary
        let mut end = 50;
        while end > 0 && !text.is_char_boundary(end) { end -= 1; }
        &text[..end]
    };
    escape_one_line(text)
}

// ============================================================================
// LEX MODE — True lexer oracle via oxc Lexer
// ============================================================================

fn cmd_lex(source: &str, source_type: SourceType, allocator: &Allocator) {
    println!("=== TOKENS ===");
    let mut lexer = Lexer::new_for_benchmarks(allocator, source, source_type);
    let mut tokens: Vec<(u32, u32, Kind)> = Vec::new();

    let mut token = lexer.first_token();
    loop {
        let kind = token.kind();
        if kind == Kind::Eof {
            tokens.push((token.start(), token.end(), kind));
            break;
        }
        // Skip whitespace/comments (should not appear, but guard anyway)
        if kind != Kind::Skip && kind != Kind::Undetermined {
            tokens.push((token.start(), token.end(), kind));
        }
        token = lexer.next_token();
    }

    println!("token_count: {}", tokens.len());
    for &(start, end, kind) in &tokens {
        let s = start as usize;
        let e = (end as usize).min(source.len());
        let text = if e > s { &source[s..e] } else { "" };
        let text = if text.len() > 80 {
            let mut trunc = 80;
            while trunc > 0 && !text.is_char_boundary(trunc) { trunc -= 1; }
            &text[..trunc]
        } else {
            text
        };
        println!("  {:?} {}:{} {}", kind, start, end, escape_one_line(text));
    }
}

// ============================================================================
// AST MODE
// ============================================================================

fn cmd_ast(program: &Program, source: &str) {
    println!("=== AST ===");
    pr(0, "Program", "", program.span);
    if let Some(hashbang) = &program.hashbang {
        pr(1, "Hashbang", &hashbang.value, hashbang.span);
    }
    for dir in &program.directives {
        let d = dir.directive.to_string();
        pr(1, "Directive", &d, dir.span);
    }
    for s in &program.body {
        print_stmt(s, 1, source);
    }
}

// ---- Statements ----

fn print_stmt(s: &Statement, d: usize, src: &str) {
    match s {
        // -- Control flow --
        Statement::BlockStatement(n) => {
            pr(d, "Block", "", n.span);
            for s in &n.body { print_stmt(s, d + 1, src); }
        }
        Statement::IfStatement(n) => {
            pr(d, "If", "", n.span);
            print_expr(&n.test, d + 1, src);
            print_stmt(&n.consequent, d + 1, src);
            if let Some(alt) = &n.alternate { print_stmt(alt, d + 1, src); }
        }
        Statement::WhileStatement(n) => {
            pr(d, "While", "", n.span);
            print_expr(&n.test, d + 1, src);
            print_stmt(&n.body, d + 1, src);
        }
        Statement::DoWhileStatement(n) => {
            pr(d, "DoWhile", "", n.span);
            print_stmt(&n.body, d + 1, src);
            print_expr(&n.test, d + 1, src);
        }
        Statement::ForStatement(n) => {
            pr(d, "For", "", n.span);
            if let Some(init) = &n.init { print_for_init(init, d + 1, src); }
            if let Some(test) = &n.test { print_expr(test, d + 1, src); }
            if let Some(update) = &n.update { print_expr(update, d + 1, src); }
            print_stmt(&n.body, d + 1, src);
        }
        Statement::ForInStatement(n) => {
            pr(d, "ForIn", "", n.span);
            print_for_left(&n.left, d + 1, src);
            print_expr(&n.right, d + 1, src);
            print_stmt(&n.body, d + 1, src);
        }
        Statement::ForOfStatement(n) => {
            pr(d, "ForOf", if n.r#await { "await" } else { "" }, n.span);
            print_for_left(&n.left, d + 1, src);
            print_expr(&n.right, d + 1, src);
            print_stmt(&n.body, d + 1, src);
        }
        Statement::SwitchStatement(n) => {
            pr(d, "Switch", "", n.span);
            print_expr(&n.discriminant, d + 1, src);
            for case in &n.cases {
                if let Some(test) = &case.test {
                    pr(d + 1, "Case", "", case.span);
                    print_expr(test, d + 2, src);
                } else {
                    pr(d + 1, "Default", "", case.span);
                }
                for s in &case.consequent { print_stmt(s, d + 2, src); }
            }
        }
        Statement::TryStatement(n) => {
            pr(d, "Try", "", n.span);
            pr(d + 1, "Block", "", n.block.span);
            for s in &n.block.body { print_stmt(s, d + 2, src); }
            if let Some(handler) = &n.handler {
                pr(d + 1, "Catch", "", handler.span);
                if let Some(param) = &handler.param {
                    print_binding(&param.pattern, d + 2, src);
                }
                pr(d + 2, "Block", "", handler.body.span);
                for s in &handler.body.body { print_stmt(s, d + 3, src); }
            }
            if let Some(fin) = &n.finalizer {
                pr(d + 1, "Finally", "", fin.span);
                for s in &fin.body { print_stmt(s, d + 2, src); }
            }
        }

        // -- Jump --
        Statement::ReturnStatement(n) => {
            pr(d, "Return", "", n.span);
            if let Some(arg) = &n.argument { print_expr(arg, d + 1, src); }
        }
        Statement::ThrowStatement(n) => {
            pr(d, "Throw", "", n.span);
            print_expr(&n.argument, d + 1, src);
        }
        Statement::BreakStatement(n) => {
            let label = n.label.as_ref().map(|l| l.name.to_string()).unwrap_or_default();
            pr(d, "Break", &label, n.span);
        }
        Statement::ContinueStatement(n) => {
            let label = n.label.as_ref().map(|l| l.name.to_string()).unwrap_or_default();
            pr(d, "Continue", &label, n.span);
        }

        // -- Other statements --
        Statement::ExpressionStatement(n) => {
            pr(d, "ExprStmt", "", n.span);
            print_expr(&n.expression, d + 1, src);
        }
        Statement::EmptyStatement(n) => pr(d, "Empty", "", n.span),
        Statement::DebuggerStatement(n) => pr(d, "Debugger", "", n.span),
        Statement::WithStatement(n) => {
            pr(d, "With", "", n.span);
            print_expr(&n.object, d + 1, src);
            print_stmt(&n.body, d + 1, src);
        }
        Statement::LabeledStatement(n) => {
            let label = n.label.name.to_string();
            pr(d, "Labeled", &label, n.span);
            print_stmt(&n.body, d + 1, src);
        }

        // -- Declarations --
        Statement::VariableDeclaration(n) => print_var_decl(n, d, src),
        Statement::FunctionDeclaration(n) => print_func(n, d, src, "FuncDecl"),
        Statement::ClassDeclaration(n) => print_class(n, d, src, "Class"),

        // -- Module --
        Statement::ImportDeclaration(n) => {
            let source_str = n.source.value.to_string();
            let mut detail = source_str;
            if let Some(phase) = &n.phase {
                detail = format!("{:?} {}", phase, detail);
            }
            pr(d, "Import", &detail, n.span);
            if let Some(specifiers) = &n.specifiers {
                for spec in specifiers {
                    match spec {
                        ImportDeclarationSpecifier::ImportSpecifier(s) => {
                            let imported = fmt_module_export_name(&s.imported);
                            let local = s.local.name.to_string();
                            let detail = if imported == local {
                                local
                            } else {
                                format!("{} as {}", imported, local)
                            };
                            pr(d + 1, "ImportSpec", &detail, s.span);
                        }
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                            let local = s.local.name.to_string();
                            pr(d + 1, "ImportDefault", &local, s.span);
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                            let local = s.local.name.to_string();
                            pr(d + 1, "ImportNamespace", &local, s.span);
                        }
                    }
                }
            }
            if let Some(wc) = &n.with_clause {
                print_with_clause(wc, d + 1);
            }
        }
        Statement::ExportNamedDeclaration(n) => {
            let source_str = n.source.as_ref().map(|s| s.value.to_string()).unwrap_or_default();
            pr(d, "ExportNamed", &source_str, n.span);
            if let Some(decl) = &n.declaration {
                print_decl(decl, d + 1, src);
            }
            for spec in &n.specifiers {
                let local = fmt_module_export_name(&spec.local);
                let exported = fmt_module_export_name(&spec.exported);
                let detail = if local == exported {
                    exported
                } else {
                    format!("{} as {}", local, exported)
                };
                pr(d + 1, "ExportSpec", &detail, spec.span);
            }
            if let Some(wc) = &n.with_clause {
                print_with_clause(wc, d + 1);
            }
        }
        Statement::ExportDefaultDeclaration(n) => {
            pr(d, "ExportDefault", "", n.span);
            match &n.declaration {
                ExportDefaultDeclarationKind::FunctionDeclaration(f) => {
                    print_func(f, d + 1, src, "FuncDecl");
                }
                ExportDefaultDeclarationKind::ClassDeclaration(c) => {
                    print_class(c, d + 1, src, "Class");
                }
                other => {
                    if let Some(e) = other.as_expression() {
                        print_expr(e, d + 1, src);
                    } else {
                        pr_unsupported(d + 1, "?ExportDefault", &snip(src, n.span), n.span);
                    }
                }
            }
        }
        Statement::ExportAllDeclaration(n) => {
            let source_str = n.source.value.to_string();
            let detail = if let Some(exported) = &n.exported {
                let alias = fmt_module_export_name(exported);
                format!("* as {} from {}", alias, source_str)
            } else {
                source_str
            };
            pr(d, "ExportAll", &detail, n.span);
            if let Some(wc) = &n.with_clause {
                print_with_clause(wc, d + 1);
            }
        }

        // -- TS (skip) --
        _ => pr_unsupported(d, "?Stmt", &snip(src, s.span()), s.span()),
    }
}

// ---- Expressions ----

fn print_expr(e: &Expression, d: usize, src: &str) {
    match e {
        // -- Literals / leaves --
        Expression::Identifier(n) => {
            let name = n.name.to_string();
            pr(d, "Ident", &name, n.span);
        }
        Expression::NumericLiteral(n) => pr(d, "NumLit", &snip(src, n.span), n.span),
        Expression::StringLiteral(n) => pr(d, "StrLit", &snip(src, n.span), n.span),
        Expression::BooleanLiteral(n) => pr(d, if n.value { "true" } else { "false" }, "", n.span),
        Expression::NullLiteral(n) => pr(d, "null", "", n.span),
        Expression::BigIntLiteral(n) => pr(d, "BigInt", &snip(src, n.span), n.span),
        Expression::RegExpLiteral(n) => pr(d, "Regex", &snip(src, n.span), n.span),
        Expression::ThisExpression(n) => pr(d, "this", "", n.span),
        Expression::Super(n) => pr(d, "super", "", n.span),

        // -- Template --
        Expression::TemplateLiteral(n) => {
            pr(d, "Template", "", n.span);
            for (i, quasi) in n.quasis.iter().enumerate() {
                pr(d + 1, "Quasi", &snip(src, quasi.span), quasi.span);
                if i < n.expressions.len() {
                    print_expr(&n.expressions[i], d + 1, src);
                }
            }
        }

        // -- Operators --
        Expression::BinaryExpression(n) => {
            let op = format!("{:?}", n.operator);
            pr(d, "Binary", &op, n.span);
            print_expr(&n.left, d + 1, src);
            print_expr(&n.right, d + 1, src);
        }
        Expression::LogicalExpression(n) => {
            let op = format!("{:?}", n.operator);
            pr(d, "Logical", &op, n.span);
            print_expr(&n.left, d + 1, src);
            print_expr(&n.right, d + 1, src);
        }
        Expression::UnaryExpression(n) => {
            let op = format!("{:?}", n.operator);
            pr(d, "Unary", &op, n.span);
            print_expr(&n.argument, d + 1, src);
        }
        Expression::UpdateExpression(n) => {
            let detail = format!("{:?} {}", n.operator, if n.prefix { "prefix" } else { "postfix" });
            pr(d, "Update", &detail, n.span);
            print_simple_target(&n.argument, d + 1, src);
        }
        Expression::AssignmentExpression(n) => {
            let op = format!("{:?}", n.operator);
            pr(d, "Assign", &op, n.span);
            print_assign_target(&n.left, d + 1, src);
            print_expr(&n.right, d + 1, src);
        }
        Expression::ConditionalExpression(n) => {
            pr(d, "Ternary", "", n.span);
            print_expr(&n.test, d + 1, src);
            print_expr(&n.consequent, d + 1, src);
            print_expr(&n.alternate, d + 1, src);
        }

        // -- Call / member --
        Expression::CallExpression(n) => {
            pr(d, "Call", if n.optional { "?." } else { "" }, n.span);
            print_expr(&n.callee, d + 1, src);
            for a in &n.arguments { print_arg(a, d + 1, src); }
        }
        Expression::NewExpression(n) => {
            pr(d, "New", "", n.span);
            print_expr(&n.callee, d + 1, src);
            for a in &n.arguments { print_arg(a, d + 1, src); }
        }
        Expression::StaticMemberExpression(n) => {
            let prop = n.property.name.to_string();
            let detail = if n.optional { format!("?.{}", prop) } else { prop };
            pr(d, "Member", &detail, n.span);
            print_expr(&n.object, d + 1, src);
        }
        Expression::ComputedMemberExpression(n) => {
            pr(d, "Index", if n.optional { "?.[]" } else { "[]" }, n.span);
            print_expr(&n.object, d + 1, src);
            print_expr(&n.expression, d + 1, src);
        }
        Expression::PrivateFieldExpression(n) => {
            let name = n.field.name.to_string();
            pr(d, "PrivateField", &name, n.span);
            print_expr(&n.object, d + 1, src);
        }

        // -- Collections --
        Expression::ArrayExpression(n) => {
            pr(d, "Array", "", n.span);
            for elem in &n.elements { print_arr_elem(elem, d + 1, src); }
        }
        Expression::ObjectExpression(n) => {
            pr(d, "Object", "", n.span);
            for prop in &n.properties { print_obj_prop(prop, d + 1, src); }
        }

        // -- Functions / classes --
        Expression::FunctionExpression(n) => print_func(n, d, src, "FuncExpr"),
        Expression::ArrowFunctionExpression(n) => {
            let mut flags = String::new();
            if n.r#async { flags.push_str("async "); }
            if n.expression { flags.push_str("expr"); } else { flags.push_str("block"); }
            pr(d, "Arrow", flags.trim(), n.span);
            print_formal_params(&n.params, d + 1, src);
            for dir in &n.body.directives {
                let dv = dir.directive.to_string();
                pr(d + 1, "Directive", &dv, dir.span);
            }
            for s in &n.body.statements { print_stmt(s, d + 1, src); }
        }
        Expression::ClassExpression(n) => print_class(n, d, src, "ClassExpr"),

        // -- Other --
        Expression::SequenceExpression(n) => {
            pr(d, "Sequence", "", n.span);
            for e in &n.expressions { print_expr(e, d + 1, src); }
        }
        Expression::AwaitExpression(n) => {
            pr(d, "Await", "", n.span);
            print_expr(&n.argument, d + 1, src);
        }
        Expression::YieldExpression(n) => {
            pr(d, "Yield", if n.delegate { "*" } else { "" }, n.span);
            if let Some(arg) = &n.argument { print_expr(arg, d + 1, src); }
        }
        Expression::TaggedTemplateExpression(n) => {
            pr(d, "TaggedTemplate", "", n.span);
            print_expr(&n.tag, d + 1, src);
            pr(d + 1, "Template", "", n.quasi.span);
            for (i, quasi) in n.quasi.quasis.iter().enumerate() {
                pr(d + 2, "Quasi", &snip(src, quasi.span), quasi.span);
                if i < n.quasi.expressions.len() {
                    print_expr(&n.quasi.expressions[i], d + 2, src);
                }
            }
        }
        Expression::ImportExpression(n) => {
            pr(d, "ImportExpr", "", n.span);
            print_expr(&n.source, d + 1, src);
            if let Some(opts) = &n.options {
                pr(d + 1, "ImportOptions", "", opts.span());
                print_expr(opts, d + 2, src);
            }
        }
        Expression::ChainExpression(n) => {
            pr(d, "Chain", "", n.span);
            print_chain_elem(&n.expression, d + 1, src);
        }
        Expression::MetaProperty(n) => {
            let detail = format!("{}.{}", n.meta.name, n.property.name);
            pr(d, "MetaProperty", &detail, n.span);
        }
        Expression::ParenthesizedExpression(n) => {
            // Skip paren wrapper — our C parser doesn't produce paren nodes
            print_expr(&n.expression, d, src);
        }
        Expression::PrivateInExpression(n) => {
            let name = n.left.name.to_string();
            pr(d, "PrivateIn", &name, n.span);
            print_expr(&n.right, d + 1, src);
        }

        // -- TS / JSX / V8 (skip) --
        _ => pr_unsupported(d, "?Expr", &snip(src, e.span()), e.span()),
    }
}

// ---- Shared structure printers ----

fn print_func(f: &Function, d: usize, src: &str, label: &str) {
    let mut flags = String::new();
    if f.r#async { flags.push_str("async "); }
    if f.generator { flags.push_str("* "); }
    if let Some(id) = &f.id {
        flags.push_str(&id.name.to_string());
    }
    pr(d, label, flags.trim(), f.span);
    print_formal_params(&f.params, d + 1, src);
    if let Some(body) = &f.body {
        for dir in &body.directives {
            let dv = dir.directive.to_string();
            pr(d + 1, "Directive", &dv, dir.span);
        }
        for s in &body.statements { print_stmt(s, d + 1, src); }
    }
}

fn print_class(c: &Class, d: usize, src: &str, label: &str) {
    let name = c.id.as_ref().map(|id| id.name.to_string()).unwrap_or_default();
    pr(d, label, &name, c.span);
    if let Some(super_class) = &c.super_class {
        pr(d + 1, "Extends", "", super_class.span());
        print_expr(super_class, d + 2, src);
    }
    for elem in &c.body.body { print_class_elem(elem, d + 1, src); }
}

fn print_formal_params(fp: &FormalParameters, d: usize, src: &str) {
    if fp.items.is_empty() && fp.rest.is_none() { return; }
    pr(d, "Params", "", fp.span);
    for param in &fp.items {
        print_binding(&param.pattern, d + 1, src);
    }
    if let Some(rest) = &fp.rest {
        pr(d + 1, "Rest", "", rest.span);
        print_binding(&rest.rest.argument, d + 2, src);
    }
}

fn print_binding(pat: &BindingPattern, d: usize, src: &str) {
    match pat {
        BindingPattern::BindingIdentifier(id) => {
            let name = id.name.to_string();
            pr(d, "Ident", &name, id.span);
        }
        BindingPattern::ObjectPattern(p) => {
            pr(d, "ObjPattern", "", p.span);
            for prop in &p.properties {
                pr(d + 1, "BindProp", if prop.shorthand { "shorthand" } else { "" }, prop.span);
                print_prop_key(&prop.key, d + 2, src);
                print_binding(&prop.value, d + 2, src);
            }
            if let Some(rest) = &p.rest {
                pr(d + 1, "Rest", "", rest.span);
                print_binding(&rest.argument, d + 2, src);
            }
        }
        BindingPattern::ArrayPattern(p) => {
            pr(d, "ArrPattern", "", p.span);
            for elem in &p.elements {
                match elem {
                    Some(e) => print_binding(e, d + 1, src),
                    None => pr(d + 1, "Elision", "", p.span),
                }
            }
            if let Some(rest) = &p.rest {
                pr(d + 1, "Rest", "", rest.span);
                print_binding(&rest.argument, d + 2, src);
            }
        }
        BindingPattern::AssignmentPattern(p) => {
            pr(d, "AssignPattern", "", p.span);
            print_binding(&p.left, d + 1, src);
            print_expr(&p.right, d + 1, src);
        }
    }
}

fn print_var_decl(n: &VariableDeclaration, d: usize, src: &str) {
    let kind = format!("{:?}", n.kind).to_lowercase();
    pr(d, "VarDecl", &kind, n.span);
    for decl in &n.declarations {
        pr(d + 1, "Declarator", "", decl.span);
        print_binding(&decl.id, d + 2, src);
        if let Some(init) = &decl.init { print_expr(init, d + 2, src); }
    }
}

fn print_arg(a: &Argument, d: usize, src: &str) {
    match a {
        Argument::SpreadElement(e) => {
            pr(d, "Spread", "", e.span);
            print_expr(&e.argument, d + 1, src);
        }
        _ => {
            if let Some(e) = a.as_expression() {
                print_expr(e, d, src);
            }
        }
    }
}

fn print_arr_elem(elem: &ArrayExpressionElement, d: usize, src: &str) {
    match elem {
        ArrayExpressionElement::SpreadElement(e) => {
            pr(d, "Spread", "", e.span);
            print_expr(&e.argument, d + 1, src);
        }
        ArrayExpressionElement::Elision(e) => {
            pr(d, "Elision", "", e.span);
        }
        _ => {
            if let Some(e) = elem.as_expression() {
                print_expr(e, d, src);
            } else {
                pr_unsupported(d, "?ArrElem", "", elem.span());
            }
        }
    }
}

fn print_obj_prop(prop: &ObjectPropertyKind, d: usize, src: &str) {
    match prop {
        ObjectPropertyKind::ObjectProperty(p) => {
            let mut flags = String::new();
            if p.shorthand { flags.push_str("shorthand "); }
            if p.computed { flags.push_str("computed "); }
            if p.method { flags.push_str("method "); }
            match p.kind {
                PropertyKind::Get => flags.push_str("get "),
                PropertyKind::Set => flags.push_str("set "),
                PropertyKind::Init => {}
            }
            pr(d, "Property", flags.trim(), p.span);
            print_prop_key(&p.key, d + 1, src);
            if !p.shorthand {
                print_expr(&p.value, d + 1, src);
            }
        }
        ObjectPropertyKind::SpreadProperty(e) => {
            pr(d, "Spread", "", e.span);
            print_expr(&e.argument, d + 1, src);
        }
    }
}

fn print_class_elem(elem: &ClassElement, d: usize, src: &str) {
    match elem {
        ClassElement::MethodDefinition(m) => {
            let mut flags = String::new();
            if m.r#static { flags.push_str("static "); }
            match m.kind {
                MethodDefinitionKind::Constructor => flags.push_str("constructor "),
                MethodDefinitionKind::Get => flags.push_str("get "),
                MethodDefinitionKind::Set => flags.push_str("set "),
                MethodDefinitionKind::Method => {}
            }
            if m.computed { flags.push_str("computed "); }
            pr(d, "Method", flags.trim(), m.span);
            print_prop_key(&m.key, d + 1, src);
            print_func(&m.value, d + 1, src, "Body");
        }
        ClassElement::PropertyDefinition(p) => {
            let mut flags = String::new();
            if p.r#static { flags.push_str("static "); }
            if p.computed { flags.push_str("computed "); }
            pr(d, "ClassProp", flags.trim(), p.span);
            print_prop_key(&p.key, d + 1, src);
            if let Some(value) = &p.value { print_expr(value, d + 1, src); }
        }
        ClassElement::AccessorProperty(p) => {
            let mut flags = String::new();
            if p.r#static { flags.push_str("static "); }
            if p.computed { flags.push_str("computed "); }
            pr(d, "Accessor", flags.trim(), p.span);
            print_prop_key(&p.key, d + 1, src);
            if let Some(value) = &p.value { print_expr(value, d + 1, src); }
        }
        ClassElement::StaticBlock(b) => {
            pr(d, "StaticBlock", "", b.span);
            for s in &b.body { print_stmt(s, d + 1, src); }
        }
        _ => pr_unsupported(d, "?ClassElem", "", elem.span()),
    }
}

fn print_prop_key(key: &PropertyKey, d: usize, src: &str) {
    match key {
        PropertyKey::StaticIdentifier(id) => {
            let name = id.name.to_string();
            pr(d, "Ident", &name, id.span);
        }
        PropertyKey::PrivateIdentifier(id) => {
            let name = id.name.to_string();
            pr(d, "PrivateIdent", &name, id.span);
        }
        _ => {
            // Computed key — try as expression
            if let Some(e) = key.as_expression() {
                print_expr(e, d, src);
            } else {
                pr_unsupported(d, "?Key", "", key.span());
            }
        }
    }
}

fn print_chain_elem(elem: &ChainElement, d: usize, src: &str) {
    match elem {
        ChainElement::CallExpression(n) => {
            pr(d, "Call", "?.", n.span);
            print_expr(&n.callee, d + 1, src);
            for a in &n.arguments { print_arg(a, d + 1, src); }
        }
        ChainElement::ComputedMemberExpression(n) => {
            pr(d, "Index", "?.[]", n.span);
            print_expr(&n.object, d + 1, src);
            print_expr(&n.expression, d + 1, src);
        }
        ChainElement::StaticMemberExpression(n) => {
            let prop = n.property.name.to_string();
            let detail = format!("?.{}", prop);
            pr(d, "Member", &detail, n.span);
            print_expr(&n.object, d + 1, src);
        }
        ChainElement::PrivateFieldExpression(n) => {
            let name = n.field.name.to_string();
            pr(d, "PrivateField", &format!("?.{}", name), n.span);
            print_expr(&n.object, d + 1, src);
        }
        _ => pr_unsupported(d, "?Chain", "", elem.span()),
    }
}

fn print_for_init(init: &ForStatementInit, d: usize, src: &str) {
    match init {
        ForStatementInit::VariableDeclaration(v) => print_var_decl(v, d, src),
        _ => {
            if let Some(e) = init.as_expression() {
                print_expr(e, d, src);
            } else {
                pr_unsupported(d, "?ForInit", &snip(src, init.span()), init.span());
            }
        }
    }
}

fn print_for_left(left: &ForStatementLeft, d: usize, src: &str) {
    match left {
        ForStatementLeft::VariableDeclaration(v) => print_var_decl(v, d, src),
        ForStatementLeft::AssignmentTargetIdentifier(id) => {
            let name = id.name.to_string();
            pr(d, "Ident", &name, id.span);
        }
        ForStatementLeft::ComputedMemberExpression(m) => {
            pr(d, "Index", "[]", m.span);
            print_expr(&m.object, d + 1, src);
            print_expr(&m.expression, d + 1, src);
        }
        ForStatementLeft::StaticMemberExpression(m) => {
            let prop = m.property.name.to_string();
            pr(d, "Member", &prop, m.span);
            print_expr(&m.object, d + 1, src);
        }
        ForStatementLeft::PrivateFieldExpression(n) => {
            let name = n.field.name.to_string();
            pr(d, "PrivateField", &name, n.span);
            print_expr(&n.object, d + 1, src);
        }
        ForStatementLeft::ArrayAssignmentTarget(a) => print_array_assign_target(a, d, src),
        ForStatementLeft::ObjectAssignmentTarget(o) => print_object_assign_target(o, d, src),
        _ => pr_unsupported(d, "?ForLeft", "", left.span()),
    }
}

fn print_assign_target(t: &AssignmentTarget, d: usize, src: &str) {
    match t {
        AssignmentTarget::AssignmentTargetIdentifier(id) => {
            let name = id.name.to_string();
            pr(d, "Ident", &name, id.span);
        }
        AssignmentTarget::ComputedMemberExpression(m) => {
            pr(d, "Index", "[]", m.span);
            print_expr(&m.object, d + 1, src);
            print_expr(&m.expression, d + 1, src);
        }
        AssignmentTarget::StaticMemberExpression(m) => {
            let prop = m.property.name.to_string();
            pr(d, "Member", &prop, m.span);
            print_expr(&m.object, d + 1, src);
        }
        AssignmentTarget::PrivateFieldExpression(n) => {
            let name = n.field.name.to_string();
            pr(d, "PrivateField", &name, n.span);
            print_expr(&n.object, d + 1, src);
        }
        AssignmentTarget::ArrayAssignmentTarget(a) => print_array_assign_target(a, d, src),
        AssignmentTarget::ObjectAssignmentTarget(o) => print_object_assign_target(o, d, src),
        _ => pr_unsupported(d, "?Target", "", t.span()),
    }
}

fn print_array_assign_target(a: &ArrayAssignmentTarget, d: usize, src: &str) {
    pr(d, "ArrPattern", "", a.span);
    for elem in &a.elements {
        match elem {
            Some(e) => print_assign_target_maybe_default(e, d + 1, src),
            None => pr(d + 1, "Elision", "", a.span),
        }
    }
    if let Some(rest) = &a.rest {
        pr(d + 1, "Rest", "", rest.span);
        print_assign_target(&rest.target, d + 2, src);
    }
}

fn print_object_assign_target(o: &ObjectAssignmentTarget, d: usize, src: &str) {
    pr(d, "ObjPattern", "", o.span);
    for prop in &o.properties {
        match prop {
            AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(p) => {
                let name = p.binding.name.to_string();
                pr(d + 1, "BindProp", &format!("shorthand {}", name), p.span);
                if let Some(init) = &p.init {
                    print_expr(init, d + 2, src);
                }
            }
            AssignmentTargetProperty::AssignmentTargetPropertyProperty(p) => {
                pr(d + 1, "BindProp", if p.computed { "computed" } else { "" }, p.span);
                print_prop_key(&p.name, d + 2, src);
                print_assign_target_maybe_default(&p.binding, d + 2, src);
            }
        }
    }
    if let Some(rest) = &o.rest {
        pr(d + 1, "Rest", "", rest.span);
        print_assign_target(&rest.target, d + 2, src);
    }
}

fn print_assign_target_maybe_default(t: &AssignmentTargetMaybeDefault, d: usize, src: &str) {
    match t {
        AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(awd) => {
            pr(d, "AssignDefault", "", awd.span);
            print_assign_target(&awd.binding, d + 1, src);
            print_expr(&awd.init, d + 1, src);
        }
        _ => {
            if let Some(target) = t.as_assignment_target() {
                print_assign_target(target, d, src);
            } else {
                pr_unsupported(d, "?MaybeDefault", "", t.span());
            }
        }
    }
}

fn print_simple_target(t: &SimpleAssignmentTarget, d: usize, src: &str) {
    match t {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
            let name = id.name.to_string();
            pr(d, "Ident", &name, id.span);
        }
        SimpleAssignmentTarget::ComputedMemberExpression(m) => {
            pr(d, "Index", "[]", m.span);
            print_expr(&m.object, d + 1, src);
            print_expr(&m.expression, d + 1, src);
        }
        SimpleAssignmentTarget::StaticMemberExpression(m) => {
            let prop = m.property.name.to_string();
            pr(d, "Member", &prop, m.span);
            print_expr(&m.object, d + 1, src);
        }
        SimpleAssignmentTarget::PrivateFieldExpression(n) => {
            let name = n.field.name.to_string();
            pr(d, "PrivateField", &name, n.span);
            print_expr(&n.object, d + 1, src);
        }
        _ => pr_unsupported(d, "?SimpleTarget", "", t.span()),
    }
}

fn print_decl(decl: &Declaration, d: usize, src: &str) {
    match decl {
        Declaration::VariableDeclaration(n) => print_var_decl(n, d, src),
        Declaration::FunctionDeclaration(n) => print_func(n, d, src, "FuncDecl"),
        Declaration::ClassDeclaration(n) => print_class(n, d, src, "Class"),
        _ => pr_unsupported(d, "?Decl", "", decl.span()),
    }
}

fn fmt_module_export_name(name: &ModuleExportName) -> String {
    match name {
        ModuleExportName::IdentifierName(id) => id.name.to_string(),
        ModuleExportName::IdentifierReference(id) => id.name.to_string(),
        ModuleExportName::StringLiteral(s) => format!("\"{}\"", s.value),
    }
}

fn print_with_clause(wc: &WithClause, d: usize) {
    let keyword = match wc.keyword {
        WithClauseKeyword::With => "with",
        WithClauseKeyword::Assert => "assert",
    };
    pr(d, "WithClause", keyword, wc.span);
    for attr in &wc.with_entries {
        let key = match &attr.key {
            ImportAttributeKey::Identifier(id) => id.name.to_string(),
            ImportAttributeKey::StringLiteral(s) => format!("\"{}\"", s.value),
        };
        let detail = format!("{}: \"{}\"", key, attr.value.value);
        pr(d + 1, "Attr", &detail, attr.span);
    }
}

// ============================================================================
// MINIFY / MANGLE MODE
// ============================================================================

fn no_comments() -> CommentOptions {
    CommentOptions {
        normal: false,
        jsdoc: false,
        annotation: false,
        legal: LegalComment::None,
    }
}

fn cmd_minify(program: &Program, mangle: bool) {
    if mangle {
        let mr = Mangler::new()
            .with_options(MangleOptions {
                top_level: Some(true),
                ..Default::default()
            })
            .build(program);

        let code = Codegen::new()
            .with_options(CodegenOptions {
                minify: true,
                comments: no_comments(),
                ..Default::default()
            })
            .with_scoping(Some(mr.scoping))
            .with_private_member_mappings(Some(mr.class_private_mappings))
            .build(program)
            .code;

        print!("{code}");
    } else {
        let code = Codegen::new()
            .with_options(CodegenOptions {
                minify: true,
                comments: no_comments(),
                ..Default::default()
            })
            .build(program)
            .code;

        print!("{code}");
    }
}

// ============================================================================
// SCOPE MODE — Per-reference resolution
// ============================================================================

fn cmd_scope(program: &Program) {
    let semantic_ret = SemanticBuilder::new()
        .with_check_syntax_error(false)
        .build(program);

    if !semantic_ret.errors.is_empty() {
        eprintln!("Semantic errors:");
        for e in &semantic_ret.errors {
            eprintln!("  {e}");
        }
    }

    let semantic = &semantic_ret.semantic;
    let scoping = semantic.scoping();

    println!("=== SCOPE ANALYSIS ===");
    println!("scopes: {}", scoping.scopes_len());
    println!("bindings: {}", scoping.symbols_len());
    println!();

    // Print each binding with per-reference resolution
    for symbol_id in scoping.symbol_ids() {
        let name = scoping.symbol_name(symbol_id);
        let scope_id = scoping.symbol_scope_id(symbol_id);
        let flags = scoping.symbol_flags(symbol_id);
        let ref_ids = scoping.get_resolved_reference_ids(symbol_id);
        let ref_count = ref_ids.len();

        println!(
            "  {:?} \"{}\" scope={:?} flags={:?} refs={}",
            symbol_id, name, scope_id, flags, ref_count
        );

        // Print each resolved reference with span and read/write flags
        for reference in semantic.symbol_references(symbol_id) {
            let span = semantic.reference_span(reference);
            let ref_flags = reference.flags();
            println!(
                "    ref {}:{} {:?}",
                span.start, span.end, ref_flags
            );
        }
    }

    // Print unresolved references (globals)
    println!();
    println!("unresolved:");
    for (name, ref_ids) in scoping.root_unresolved_references() {
        println!("  \"{}\" refs={}", name, ref_ids.len());
        for &ref_id in ref_ids.iter() {
            let reference = scoping.get_reference(ref_id);
            let span = semantic.reference_span(reference);
            let ref_flags = reference.flags();
            println!(
                "    ref {}:{} {:?}",
                span.start, span.end, ref_flags
            );
        }
    }
}
