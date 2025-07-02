use clap::Parser;
use anyhow::Result;
use regex::Regex;
use tree_sitter::{Language, Parser as TreeSitterParser};
use walkdir::WalkDir;
use colored::*;
use dirs::home_dir;
extern crate tree_sitter_php;

unsafe extern "C" { fn tree_sitter_php() -> Language; }

/// Search PHP code for strings inside functions and classes
#[derive(Parser, Debug)]
#[command(name = "phrep")]
#[command(about = "Grep inside PHP functions/methods.", version)]
struct Cli {
    /// Search query
    query: String,
    /// Directory to search recursively (default is current directory)
    #[arg(long, value_name = "DIR", default_value = ".")]
    #[arg(short, value_name = "DIR")]
    dir:String,

    /// File to search (default is all .php files)
    #[arg(long, value_name = "FILE", default_value = ".php")]
    #[arg(short, value_name = "FILE")]
    file: String,

    /// Search class properties only (default is false)
    #[arg(long, value_name = "PROPERTIES", default_value_t = false)]
    #[arg(short, value_name = "PROPERTIES")]
    properties: bool,
}

fn main() -> Result<()> {
    let args: Cli = Cli::parse();
    if args.query.is_empty() {
        eprintln!("Error: Query cannot be empty.");
        return Err(anyhow::anyhow!("Query cannot be empty"));
    }

    if args.properties {
        println!("Searching for class properties matching: {}", args.query);
    } else {
        basic_search(&args.query, &args.dir, &args.file)?;
    }

    println!("Search completed successfully.");
    Ok(())
}

// Just a basic search that searches within methods in a class
fn basic_search(query: &str, dir: &str, file: &str) -> Result<()> {
    let pattern = Regex::new(query);
    let mut parser = TreeSitterParser::new();
    parser.set_language(unsafe { tree_sitter_php() })?;
    if let Err(e) = pattern {
        eprintln!("Invalid regex pattern: {}", e);
        return Err(anyhow::anyhow!("Invalid regex pattern"));
    }

    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("php"))
        .filter(|e| e.file_name().to_string_lossy().contains(file)) {
        
        let path = entry.path();
        if path.is_file() {
            let content = std::fs::read_to_string(path)?;
            let tree = parser.parse(&content, None).unwrap();
            let root_node = tree.root_node();
            for node in root_node.children(&mut tree.walk()) {
                if node.kind() == "class_declaration" {
                    let class_body = node.child_by_field_name("body");
                    let cursor = class_body.unwrap();
                    for method in class_body.unwrap().named_children(&mut cursor.walk()) {
                        if method.kind() == "method_declaration" || method.kind() == "function_declaration" {
                            let name_node = method.child_by_field_name("name");
                            let body_node = method.child_by_field_name("body");
                            if let (Some(name_node), Some(body_node)) = (name_node, body_node) {
                                let func_name = name_node.utf8_text(content.as_bytes()).unwrap_or("unknown");

                                let body_text = body_node.utf8_text(content.as_bytes()).unwrap_or("");
                                let start_row = body_node.start_position().row;
                                for (i, line) in body_text.lines().enumerate() {
                                    if pattern.clone()?.is_match(line) {
                                        let mut filename = path.display().to_string();
                                        if let Some(home_dir) = home_dir() {
                                            if let Some(home_dir_str) = home_dir.to_str() {
                                                if filename.starts_with(home_dir_str) {
                                                    filename = filename.replace(home_dir_str, "~");
                                                }
                                            }
                                        }
                                        if filename.starts_with("./") {
                                            filename = filename[2..].to_string();
                                        }
                                        let file_name_styled = filename.bold().blue();
                                        let func_name_styled = func_name.bold().yellow();

                                        println!("{}: {}():{} → {}", file_name_styled, func_name_styled, start_row + i + 1, line.trim());
                                    }
                                }
                            }
                        }
                    }
                } else if node.kind() == "function_declaration" {
                    let name_node = node.child_by_field_name("name");
                    let body_node = node.child_by_field_name("body");
                    if let (Some(name_node), Some(body_node)) = (name_node, body_node) {
                        let func_name = name_node.utf8_text(content.as_bytes()).unwrap_or("unknown");

                        let body_text = body_node.utf8_text(content.as_bytes()).unwrap_or("");
                        let start_row = body_node.start_position().row;
                        for (i, line) in body_text.lines().enumerate() {
                            if pattern.clone()?.is_match(line) {
                                let mut filename = path.display().to_string();
                                if let Some(home_dir) = home_dir() {
                                    if let Some(home_dir_str) = home_dir.to_str() {
                                        if filename.starts_with(home_dir_str) {
                                            filename = filename.replace(home_dir_str, "~");
                                        }
                                    }
                                }
                                if filename.starts_with("./") {
                                    filename = filename[2..].to_string();
                                }
                                let file_name_styled = filename.bold().blue();
                                let func_name_styled = func_name.bold().yellow();

                                println!("{}: {}():{} → {}", file_name_styled, func_name_styled, start_row + i + 1, line.trim());
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
