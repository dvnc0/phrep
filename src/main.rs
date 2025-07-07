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
#[command(about = "Grep style search inside PHP functions/methods. Basic search searches within methods and returns line and method information", version)]
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

    /// Print full method body in basic search
    #[arg(long, value_name = "PRINT_METHOD", default_value_t = false)]
    #[arg(short, value_name = "PRINT_METHOD")]
    print_method: bool,

    /// Mimic grep search (default is false)
    #[arg(long, value_name = "GREP", default_value_t = false)]
    #[arg(short, value_name = "GREP")]
    grep: bool,

    /// Return the entire method if method name matches the query
    #[arg(long, value_name = "METHOD_SEARCH", default_value_t = false)]
    #[arg(short, value_name = "METHOD_SEARCH")]
    method_search: bool,
}

#[derive(Debug)]
enum SearchMode {
    Basic,
    Grep,
    MethodSearch,
}

impl From<&Cli> for SearchMode {
    fn from(args: &Cli) -> Self {
        if args.grep {
            SearchMode::Grep
        } else if args.method_search {
            SearchMode::MethodSearch
        } else {
            SearchMode::Basic
        }
    }
}

fn main() -> Result<()> {
    let args: Cli = Cli::parse();
    
    validate_args(&args)?;

    let search_mode = SearchMode::from(&args);

    search(&args.query, &args.dir, &args.file, search_mode, &args.print_method)?;

    println!("Search completed successfully.");
    Ok(())
}

fn validate_args(args: &Cli) -> Result<()> {
    if args.query.is_empty() {
        eprintln!("Error: Query cannot be empty.");
        return Err(anyhow::anyhow!("Query cannot be empty"));
    }

    if args.grep && args.method_search {
        eprintln!("Error: Cannot use both --grep and --method-search at the same time.");
        return Err(anyhow::anyhow!("Cannot use both --grep and --method-search at the same time"));
    }

    Ok(())
}

fn search(query: &str, dir: &str, file: &str, mode: SearchMode, print_method: &bool) -> Result<()> {
    match mode {
        SearchMode::Basic => basic_search(query, dir, file, print_method),
        SearchMode::Grep => grep_search(query, dir, file),
        SearchMode::MethodSearch => method_search(query, dir, file),
    }
}

fn format_filename(path: &std::path::Path) -> String {
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

    filename
}

fn search_in_function_body(content: &str, pattern: &Regex, parser: &mut TreeSitterParser, path: &std::path::Path, print_method: &bool) -> Result<()> {
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
                            if pattern.clone().is_match(line) {
                                let filename = format_filename(path);
                                let file_name_styled = filename.bold().blue();
                                let func_name_styled = func_name.bold().yellow();

                                if *print_method {
                                    let body_text_styled = body_text.replace(pattern.as_str(), &format!("{}", pattern.as_str().bold().red()));
                                    println!("{}:{}: {}() → {}", file_name_styled, start_row + i + 1, func_name_styled, body_text_styled.trim());
                                } else {
                                    let line_styled = line.replace(pattern.as_str(), &format!("{}", pattern.as_str().bold().red()));
                                    println!("{}:{}: {}() → {}", file_name_styled, start_row + i + 1, func_name_styled, line_styled.trim());
                                }
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
                    if pattern.clone().is_match(line) {
                        let filename = format_filename(path);
                        let file_name_styled = filename.bold().blue();
                        let func_name_styled = func_name.bold().yellow();

                        println!("{}:{}: {}() → {}", file_name_styled, start_row + i + 1, func_name_styled, line.trim());
                    }
                }
            }
        }
    }
    Ok(())
}

// Just a basic search that searches within methods in a class
fn basic_search(query: &str, dir: &str, file: &str, print_method: &bool) -> Result<()> {
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
            let reg_pattern = &pattern.clone().unwrap();
            search_in_function_body(&content, &reg_pattern, &mut parser, &path, print_method)?;
        }
    }

    Ok(())
}

// Searches method name match and prints the entire method body
// This is useful for finding methods by name and seeing their implementation
fn method_search(query: &str, dir: &str, file: &str) -> Result<()> {
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
                                    if func_name.contains(query) {
                                        let filename = format_filename(path);
                                        let file_name_styled = filename.bold().blue();
                                        let func_name_styled = func_name.bold().yellow();
                                       
                                        let params = method.child_by_field_name("parameters");
                                        let params_text = if let Some(params) = params {
                                            params.utf8_text(content.as_bytes()).unwrap_or("")
                                        } else {
                                            ""
                                        };
                                        let params_styled = params_text.bold().green();

                                        let return_type = method.child_by_field_name("return_type");
                                        let return_type_text = if let Some(return_type) = return_type {
                                            return_type.utf8_text(content.as_bytes()).unwrap_or("")
                                        } else {
                                            ""
                                        };
                                        let return_type_styled = return_type_text.bold().magenta();

                                        println!("{}:{}: {}{}:{} → {}", file_name_styled, start_row + 1, func_name_styled, params_styled, return_type_styled, body_text.trim());

                                    }
                                }
                            }
                        }
                    }
                }
            }
        }


    Ok(())
}

// Mimics grep search, searching for the query in all files
fn grep_search(query: &str, dir: &str, file: &str) -> Result<()> {
    let pattern = Regex::new(query);
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
            let filename = format_filename(path);
            let file_name_styled = filename.bold().blue();
            for (i, line) in content.lines().enumerate() {
                if pattern.clone()?.is_match(line) {
                    let pattern_text = pattern.clone().unwrap();
                    let line_styled = line.replace(pattern_text.as_str(), &format!("{}", pattern_text.as_str().bold().red()));
                    println!("{}:{} → {}", file_name_styled, i + 1, line_styled.trim());
                }
            }
        }
    }

    Ok(())
}