use std::path::Path;

#[test]
fn test_sample_md_exists() {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/sample.md");
    assert!(sample.exists(), "sample.md should exist in tests/");
}

#[test]
fn test_binary_help() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_mdview"))
        .arg("--help")
        .output()
        .expect("Failed to run mdview");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.to_lowercase().contains("markdown viewer"),
        "Help should mention markdown viewer"
    );
}

#[test]
fn test_binary_version() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_mdview"))
        .arg("--version")
        .output()
        .expect("Failed to run mdview");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mdview"));
}

#[test]
fn test_binary_missing_file() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_mdview"))
        .arg("nonexistent_file.md")
        .output()
        .expect("Failed to run mdview");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Error") || stderr.contains("error"),
        "Should print error for missing file"
    );
}

// Note: removed test_binary_no_args since the app now opens a file picker 
// when no file is provided, which doesn't work in a CI environment

#[test]
fn test_sample_md_parses_without_panic() {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/sample.md");
    let content = std::fs::read_to_string(&sample).expect("Failed to read sample.md");
    let elements = mdview::parser::parse_markdown(&content);
    assert!(!elements.is_empty(), "Parsed sample.md should produce non-empty elements");
}

#[test]
fn test_other_md_exists_and_parses() {
    let other = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/other.md");
    assert!(other.exists(), "other.md should exist in tests/");
    let content = std::fs::read_to_string(&other).expect("Failed to read other.md");
    let elements = mdview::parser::parse_markdown(&content);
    assert!(!elements.is_empty(), "Parsed other.md should produce non-empty elements");
}

#[test]
fn test_parse_sample_has_headings() {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/sample.md");
    let content = std::fs::read_to_string(&sample).expect("Failed to read sample.md");
    let elements = mdview::parser::parse_markdown(&content);
    let has_heading = elements.iter().any(|e| matches!(e, mdview::parser::MdElement::Heading { .. }));
    assert!(has_heading, "sample.md should contain at least one Heading element");
}

#[test]
fn test_parse_sample_has_code_blocks() {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/sample.md");
    let content = std::fs::read_to_string(&sample).expect("Failed to read sample.md");
    let elements = mdview::parser::parse_markdown(&content);
    let has_code = elements.iter().any(|e| matches!(e, mdview::parser::MdElement::CodeBlock { .. }));
    assert!(has_code, "sample.md should contain at least one CodeBlock element");
}

#[test]
fn test_parse_sample_has_tables() {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/sample.md");
    let content = std::fs::read_to_string(&sample).expect("Failed to read sample.md");
    let elements = mdview::parser::parse_markdown(&content);
    let has_table = elements.iter().any(|e| matches!(e, mdview::parser::MdElement::Table { .. }));
    assert!(has_table, "sample.md should contain at least one Table element");
}

#[test]
fn test_parse_sample_has_lists() {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/sample.md");
    let content = std::fs::read_to_string(&sample).expect("Failed to read sample.md");
    let elements = mdview::parser::parse_markdown(&content);
    let has_list = elements.iter().any(|e| matches!(e, mdview::parser::MdElement::List { .. }));
    assert!(has_list, "sample.md should contain at least one List element");
}

#[test]
fn test_binary_invalid_flag() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_mdview"))
        .arg("--invalid-flag")
        .output()
        .expect("Failed to run mdview");
    assert!(!output.status.success(), "Binary should fail with invalid flag");
}

#[test]
fn test_strip_html_tags_from_integration() {
    assert_eq!(mdview::renderer::strip_html_tags("<b>bold</b>"), "bold");
    assert_eq!(mdview::renderer::strip_html_tags("no tags"), "no tags");
    assert_eq!(mdview::renderer::strip_html_tags("<p>hello</p> <em>world</em>"), "hello world");
    assert_eq!(mdview::renderer::strip_html_tags(""), "");
    assert_eq!(mdview::renderer::strip_html_tags("<br/>"), "");
}

#[test]
fn test_extract_toc_from_sample() {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/sample.md");
    let content = std::fs::read_to_string(&sample).expect("Failed to read sample.md");
    let elements = mdview::parser::parse_markdown(&content);
    let toc = mdview::renderer::extract_toc(&elements);
    assert!(!toc.is_empty(), "TOC should contain entries from sample.md");
    // Each entry should have (level, text, anchor)
    for (level, text, anchor) in &toc {
        assert!(*level >= 1 && *level <= 6, "Heading level should be 1-6");
        assert!(!text.is_empty(), "Heading text should not be empty");
        assert!(!anchor.is_empty(), "Heading anchor should not be empty");
    }
}

#[test]
fn test_heading_to_anchor_from_integration() {
    assert_eq!(mdview::links::heading_to_anchor("Hello World"), "hello-world");
    assert_eq!(mdview::links::heading_to_anchor("Section 1: Headings"), "section-1-headings");
    assert_eq!(mdview::links::heading_to_anchor("ALL CAPS"), "all-caps");
    assert_eq!(mdview::links::heading_to_anchor("special!@#chars"), "specialchars");
}
