use oxc_allocator::Allocator;
use oxc_codegen::{Codegen, CodegenOptions, CommentOptions, LegalComment};
use oxc_minifier::{Minifier, MinifierOptions};
use oxc_mangler::{Mangler, MangleOptions, MangleOptionsKeepNames};
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::{fs, io};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = "f.js";

    let source_text = std::fs::read_to_string(path)?;
    let source_type = SourceType::from_path(path).unwrap();
    let allocator = Allocator::with_capacity(1<<18);
    let ret = Parser::new(&allocator, &source_text, source_type).parse();
    if !ret.errors.is_empty() {
        eprintln!("Parse errors: {:?}", ret.errors);
        return Err("Failed to parse".into());
    }

    let mangler_return = Mangler::new().with_options(MangleOptions {
        top_level: true,
        keep_names: MangleOptionsKeepNames { function: false, class: false },
        debug: false,
    }).build(&ret.program);
    
    let printed = Codegen::new().with_options(CodegenOptions {
        minify: true,
        indent_width: 0,
        comments: CommentOptions {
            normal: false,
            jsdoc: false,
            annotation: false,
            legal: LegalComment::None
        },
        ..CodegenOptions::default()
    })
    .with_scoping(Some(mangler_return.scoping))
    .with_private_member_mappings(Some(mangler_return.class_private_mappings))
    .build(&ret.program)
    .code;

    println!("{printed}");

    Ok(())
}
