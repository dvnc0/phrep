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
    #[arg(long, short, value_name = "DIR", default_value = ".")]
    dir:String,

    /// File to search (default is all .php files)
    #[arg(long, short, value_name = "FILE", default_value = ".php")]
    file: String,

    /// Print full method body in basic search
    #[arg(long, short, value_name = "PRINT_METHOD", default_value_t = false, conflicts_with_all = ["grep", "method_search"])]
    print_method: bool,

    /// Mimic grep search (default is false)
    #[arg(long, short, value_name = "GREP", default_value_t = false)]
    grep: bool,

    /// Return the entire method if method name matches the query
    #[arg(long, short, value_name = "METHOD_SEARCH", default_value_t = false, conflicts_with_all = ["grep", "print_method"])]
    method_search: bool,

    /// Exclude directories from search
    #[arg(long, short, value_name = "EXCLUDE_DIRS", default_value = "vendor,cache,logs")]
    exclude_dirs: String,
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

    search(&args.query, &args.dir, &args.file, search_mode, &args.print_method, &args.exclude_dirs)?;

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

    if !args.exclude_dirs.is_empty() {
        let dirs: Vec<&str> = args.exclude_dirs.split(',').collect();
        if dirs.is_empty() || dirs.iter().any(|d| d.trim().is_empty()) {
            eprintln!("Error: Invalid exclude_dirs format. Use a comma-separated list.");
            return Err(anyhow::anyhow!("Invalid exclude_dirs format. Use a comma-separated list."));
        }
    }

    Ok(())
}

fn search(query: &str, dir: &str, file: &str, mode: SearchMode, print_method: &bool, exclude_dirs: &str) -> Result<()> {
    match mode {
        SearchMode::Basic => basic_search(query, dir, file, print_method, exclude_dirs),
        SearchMode::Grep => grep_search(query, dir, file, exclude_dirs),
        SearchMode::MethodSearch => method_search(query, dir, file, exclude_dirs),
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
    let tree = match parser.parse(&content, None) {
        Some(tree) => tree,
        None => {
            return Err(anyhow::anyhow!("Could not parse content as PHP"));
        }
    };
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
                        let func_name = match name_node.utf8_text(content.as_bytes()) {
                            Ok(name) => name,
                            Err(_) => {
                                eprintln!("Warning: Invalid UTF-8 in function name in file '{}'", path.display());
                                continue;
                            }
                        };

                        let body_text = match body_node.utf8_text(content.as_bytes()) {
                            Ok(text) => text,
                            Err(_) => {
                                eprintln!("Warning: Invalid UTF-8 in function body in file '{}'", path.display());
                                continue;
                            }
                        };
                        let start_row = body_node.start_position().row;
                        let start = body_node.start_position().row;
                        let end = body_node.end_position().row;
                        for (i, line) in content.lines().enumerate().skip(start + 1).take(end - start + 1) {
                            if pattern.is_match(line) {
                                let filename = format_filename(path);
                                let file_name_styled = filename.bold().blue();
                                let func_name_styled = func_name.bold().yellow();

                                if *print_method {
                                    if let Some(_pattern_str) = pattern.as_str().chars().next() {
                                        let body_text_styled = body_text.replace(pattern.as_str(), &format!("{}", pattern.as_str().bold().red()));
                                        println!("{}:{}: {}() → {}", file_name_styled, start_row + i + 1, func_name_styled, body_text_styled.trim());
                                    } else {
                                        println!("{}:{}: {}() → {}", file_name_styled, start_row + i + 1, func_name_styled, body_text.trim());
                                    }
                                } else {
                                    if let Some(_pattern_str) = pattern.as_str().chars().next() {
                                        let line_styled = line.replace(pattern.as_str(), &format!("{}", pattern.as_str().bold().red()));
                                        println!("{}:{}: {}() → {}", file_name_styled, start_row + i + 1, func_name_styled, line_styled.trim());
                                    } else {
                                        println!("{}:{}: {}() → {}", file_name_styled, start_row + i + 1, func_name_styled, line.trim());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Now recursively find all function_definition nodes (including nested ones)
    search_in_all_functions(&root_node, content, pattern, path, print_method)?;
    
    Ok(())
}

// Recursive function to search inside all function_definition nodes regardless of nesting
fn search_in_all_functions(node: &tree_sitter::Node, content: &str, pattern: &Regex, path: &std::path::Path, print_method: &bool) -> Result<()> {
    if node.kind() == "function_definition" {
        if let Some(name_node) = node.child_by_field_name("name") {
            let func_name = match name_node.utf8_text(content.as_bytes()) {
                Ok(name) => name,
                Err(_) => {
                    eprintln!("Warning: Invalid UTF-8 in function name in file '{}'", path.display());
                    return Ok(());
                }
            };
            
            if let Some(body_node) = node.child_by_field_name("body") {
                let body_text = match body_node.utf8_text(content.as_bytes()) {
                    Ok(text) => text,
                    Err(_) => {
                        eprintln!("Warning: Invalid UTF-8 in function body in file '{}'", path.display());
                        return Ok(());
                    }
                };
                let start_row = body_node.start_position().row;
                
                for (i, line) in body_text.lines().enumerate() {
                    if pattern.is_match(line) {
                        let filename = format_filename(path);
                        let file_name_styled = filename.bold().blue();
                        let func_name_styled = func_name.bold().yellow();

                        if *print_method {
                            if let Some(_pattern_str) = pattern.as_str().chars().next() {
                                let body_text_styled = body_text.replace(pattern.as_str(), &format!("{}", pattern.as_str().bold().red()));
                                println!("{}:{}: {}() → {}", file_name_styled, start_row + i + 1, func_name_styled, body_text_styled.trim());
                            } else {
                                println!("{}:{}: {}() → {}", file_name_styled, start_row + i + 1, func_name_styled, body_text.trim());
                            }
                        } else {
                            if let Some(_pattern_str) = pattern.as_str().chars().next() {
                                let line_styled = line.replace(pattern.as_str(), &format!("{}", pattern.as_str().bold().red()));
                                println!("{}:{}: {}() → {}", file_name_styled, start_row + i + 1, func_name_styled, line_styled.trim());
                            } else {
                                println!("{}:{}: {}() → {}", file_name_styled, start_row + i + 1, func_name_styled, line.trim());
                            }
                        }
                    }
                }
            }
        }
    }
    
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        search_in_all_functions(&child, content, pattern, path, print_method)?;
    }
    
    Ok(())
}

fn basic_search(query: &str, dir: &str, file: &str, print_method: &bool, exclude_dirs: &str) -> Result<()> {
    let pattern = Regex::new(query);
    let mut parser = TreeSitterParser::new();
    parser.set_language(unsafe { tree_sitter_php() })?;
    if let Err(e) = pattern {
        eprintln!("Invalid regex pattern: {}", e);
        return Err(anyhow::anyhow!("Invalid regex pattern"));
    }
    let exclude_dirs: Vec<&str> = exclude_dirs.split(',').map(|s| s.trim()).collect();

    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            if let Some(path_str) = e.path().to_str() {
                let relative_path = e.path().strip_prefix(dir).unwrap_or(e.path()).to_string_lossy();
                !exclude_dirs.iter().any(|excluded_dir| {
                    path_str.contains(excluded_dir) || 
                    relative_path.starts_with(excluded_dir) ||
                    path_str.ends_with(excluded_dir)
                })
            } else {
                true
            }
        })
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("php"))
        .filter(|e| e.file_name().to_string_lossy().contains(file)) {
        
        let path = entry.path();
        if path.is_file() {
            let content = match std::fs::read_to_string(path) {
                Ok(content) => content,
                Err(e) => {
                    eprintln!("Warning: Could not read file '{}': {}", path.display(), e);
                    continue;
                }
            };
            
            let reg_pattern = &pattern.clone().unwrap();
            
            if !content.lines().any(|line| reg_pattern.is_match(line)) {
                continue;
            }

            if let Err(e) = search_in_function_body(&content, &reg_pattern, &mut parser, &path, print_method) {
                eprintln!("Warning: Error processing file '{}': {}", path.display(), e);
                continue;
            }
        }
    }

    Ok(())
}

// Searches method name match and prints the entire method body
// This is useful for finding methods by name and seeing their implementation
fn method_search(query: &str, dir: &str, file: &str, exclude_dirs: &str) -> Result<()> {
    let pattern = Regex::new(query);
    let mut parser = TreeSitterParser::new();
    parser.set_language(unsafe { tree_sitter_php() })?;
    if let Err(e) = pattern {
        eprintln!("Invalid regex pattern: {}", e);
        return Err(anyhow::anyhow!("Invalid regex pattern"));
    }

    let exclude_dirs: Vec<&str> = exclude_dirs.split(',').map(|s| s.trim()).collect();

    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            if let Some(path_str) = e.path().to_str() {
                let relative_path = e.path().strip_prefix(dir).unwrap_or(e.path()).to_string_lossy();
                !exclude_dirs.iter().any(|excluded_dir| {
                    path_str.contains(excluded_dir) || 
                    relative_path.starts_with(excluded_dir) ||
                    path_str.ends_with(excluded_dir)
                })
            } else {
                true
            }
        })
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("php"))
        .filter(|e| e.file_name().to_string_lossy().contains(file)) {

        let path = entry.path();
        if path.is_file() {
            let content = match std::fs::read_to_string(path) {
                Ok(content) => content,
                Err(e) => {
                    eprintln!("Warning: Could not read file '{}': {}", path.display(), e);
                    continue;
                }
            };
            
            if !content.contains(query) {
                continue;
            }
            
            let tree = match parser.parse(&content, None) {
                Some(tree) => tree,
                None => {
                    eprintln!("Warning: Could not parse file '{}' as PHP", path.display());
                    continue;
                }
            };
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
                                let func_name = match name_node.utf8_text(content.as_bytes()) {
                                    Ok(name) => name,
                                    Err(_) => {
                                        eprintln!("Warning: Invalid UTF-8 in method name in file '{}'", path.display());
                                        continue;
                                    }
                                };

                                let body_text = match body_node.utf8_text(content.as_bytes()) {
                                    Ok(text) => text,
                                    Err(_) => {
                                        eprintln!("Warning: Invalid UTF-8 in method body in file '{}'", path.display());
                                        continue;
                                    }
                                };
                                let start_row = body_node.start_position().row;
                                if func_name.contains(query) {
                                    let filename = format_filename(path);
                                    let file_name_styled = filename.bold().blue();
                                    let func_name_styled = func_name.bold().yellow();
                                   
                                    let params_text = method.child_by_field_name("parameters")
                                        .and_then(|p| p.utf8_text(content.as_bytes()).ok())
                                        .unwrap_or("");
                                    let params_styled = params_text.bold().green();

                                    let return_type_text = method.child_by_field_name("return_type")
                                        .and_then(|r| r.utf8_text(content.as_bytes()).ok())
                                        .unwrap_or("");
                                    let return_type_styled = return_type_text.bold().magenta();

                                    println!("{}:{}: {}{}:{} → {}", file_name_styled, start_row + 1, func_name_styled, params_styled, return_type_styled, body_text.trim());
                                }
                            }
                        }
                    }
                }
            }
            
            if let Err(e) = find_all_functions(&root_node, &content, query, path) {
                eprintln!("Warning: Error processing functions in file '{}': {}", path.display(), e);
                continue;
            }
        }
    }

    Ok(())
}

// Recursive function to find all function_definition nodes regardless of nesting
fn find_all_functions(node: &tree_sitter::Node, content: &str, query: &str, path: &std::path::Path) -> Result<()> {
    // Check if this node is a function_definition
    if node.kind() == "function_definition" {
        if let Some(name_node) = node.child_by_field_name("name") {
            let func_name = match name_node.utf8_text(content.as_bytes()) {
                Ok(name) => name,
                Err(_) => {
                    eprintln!("Warning: Invalid UTF-8 in function name in file '{}'", path.display());
                    return Ok(());
                }
            };
            
            if func_name.contains(query) {
                let filename = format_filename(path);
                let file_name_styled = filename.bold().blue();
                let func_name_styled = func_name.bold().yellow();
                
                let params_text = node.child_by_field_name("parameters")
                    .and_then(|p| p.utf8_text(content.as_bytes()).ok())
                    .unwrap_or("");
                let params_styled = params_text.bold().green();

                let return_type_text = node.child_by_field_name("return_type")
                    .and_then(|r| r.utf8_text(content.as_bytes()).ok())
                    .unwrap_or("");
                let return_type_styled = return_type_text.bold().magenta();

                let body_text = node.child_by_field_name("body")
                    .and_then(|b| b.utf8_text(content.as_bytes()).ok())
                    .unwrap_or("");
                let start_row = node.start_position().row;

                println!("{}:{}: {}{}:{} → {}", 
                    file_name_styled, 
                    start_row + 1, 
                    func_name_styled, 
                    params_styled, 
                    return_type_styled, 
                    body_text.trim()
                );
            }
        }
    }
    
    // Recursively check all child nodes
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_all_functions(&child, content, query, path)?;
    }
    
    Ok(())
}

// Mimics grep search, searching for the query in all files
fn grep_search(query: &str, dir: &str, file: &str, exclude_dirs: &str) -> Result<()> {
    let pattern = Regex::new(query);
    if let Err(e) = pattern {
        eprintln!("Invalid regex pattern: {}", e);
        return Err(anyhow::anyhow!("Invalid regex pattern"));
    }

    let exclude_dirs: Vec<&str> = exclude_dirs.split(',').map(|s| s.trim()).collect();

    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            if let Some(path_str) = e.path().to_str() {
                let relative_path = e.path().strip_prefix(dir).unwrap_or(e.path()).to_string_lossy();
                !exclude_dirs.iter().any(|excluded_dir| {
                    path_str.contains(excluded_dir) || 
                    relative_path.starts_with(excluded_dir) ||
                    path_str.ends_with(excluded_dir)
                })
            } else {
                true
            }
        })
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("php"))
        .filter(|e| e.file_name().to_string_lossy().contains(file)) {
        
        let path = entry.path();
        if path.is_file() {
            let content = match std::fs::read_to_string(path) {
                Ok(content) => content,
                Err(e) => {
                    eprintln!("Warning: Could not read file '{}': {}", path.display(), e);
                    continue;
                }
            };
            let filename = format_filename(path);
            let file_name_styled = filename.bold().blue();
            for (i, line) in content.lines().enumerate() {
                if pattern.clone().unwrap().is_match(line) {
                    let pattern_ref = pattern.clone().unwrap();
                    let line_styled = line.replace(pattern_ref.as_str(), &format!("{}", pattern_ref.as_str().bold().red()));
                    println!("{}:{} → {}", file_name_styled, i + 1, line_styled.trim());
                }
            }
        }
    }

    Ok(())
}