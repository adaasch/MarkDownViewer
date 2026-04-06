use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

/// A block-level markdown element.
#[derive(Debug, Clone, PartialEq)]
pub enum MdElement {
    Heading {
        level: u8,
        content: Vec<InlineElement>,
    },
    Paragraph(Vec<InlineElement>),
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    Table {
        headers: Vec<Vec<InlineElement>>,
        rows: Vec<Vec<Vec<InlineElement>>>,
    },
    List {
        ordered: bool,
        start: Option<u64>,
        items: Vec<ListItem>,
    },
    ThematicBreak,
    BlockQuote(Vec<MdElement>),
    HtmlBlock(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub checked: Option<bool>,
    pub content: Vec<MdElement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InlineElement {
    Text(String),
    Bold(Vec<InlineElement>),
    Italic(Vec<InlineElement>),
    Strikethrough(Vec<InlineElement>),
    Code(String),
    Link {
        content: Vec<InlineElement>,
        url: String,
    },
    Image {
        alt: String,
        url: String,
    },
    Html(String),
    SoftBreak,
    HardBreak,
}

pub fn parse_markdown(input: &str) -> Vec<MdElement> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(input, options);
    let events: Vec<Event> = parser.collect();
    
    let mut pos = 0;
    parse_blocks(&events, &mut pos)
}

fn collect_html_block(events: &[Event], pos: &mut usize) -> String {
    let mut html = String::new();
    while *pos < events.len() {
        match &events[*pos] {
            Event::Html(fragment) => {
                html.push_str(fragment);
                *pos += 1;
            }
            _ => break,
        }
    }
    html
}

fn parse_blocks(events: &[Event], pos: &mut usize) -> Vec<MdElement> {
    let mut elements = Vec::new();
    
    while *pos < events.len() {
        match &events[*pos] {
            Event::Start(Tag::Heading { level, .. }) => {
                *pos += 1; // consume Start event
                let content = parse_inlines(events, pos, TagEnd::Heading(*level));
                elements.push(MdElement::Heading {
                    level: *level as u8,
                    content,
                });
                *pos += 1; // consume End event
            }
            Event::Start(Tag::Paragraph) => {
                *pos += 1; // consume Start event
                let content = parse_inlines(events, pos, TagEnd::Paragraph);
                elements.push(MdElement::Paragraph(content));
                *pos += 1; // consume End event
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                *pos += 1; // consume Start event
                let language = if info.is_empty() {
                    None
                } else {
                    Some(info.to_string())
                };
                let mut code = String::new();
                while *pos < events.len() {
                    match &events[*pos] {
                        Event::Text(text) => {
                            code.push_str(text);
                            *pos += 1;
                        }
                        Event::End(TagEnd::CodeBlock) => break,
                        _ => *pos += 1,
                    }
                }
                elements.push(MdElement::CodeBlock { language, code });
                *pos += 1; // consume End event
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Indented)) => {
                *pos += 1; // consume Start event
                let mut code = String::new();
                while *pos < events.len() {
                    match &events[*pos] {
                        Event::Text(text) => {
                            code.push_str(text);
                            *pos += 1;
                        }
                        Event::End(TagEnd::CodeBlock) => break,
                        _ => *pos += 1,
                    }
                }
                elements.push(MdElement::CodeBlock { 
                    language: None, 
                    code 
                });
                *pos += 1; // consume End event
            }
            Event::Start(Tag::List(start_number)) => {
                *pos += 1; // consume Start event
                let items = parse_list_items(events, pos);
                let ordered = start_number.is_some();
                elements.push(MdElement::List {
                    ordered,
                    start: *start_number,
                    items,
                });
                *pos += 1; // consume End event
            }
            Event::Start(Tag::Table(_)) => {
                *pos += 1; // consume Start event
                let (headers, rows) = parse_table(events, pos);
                elements.push(MdElement::Table { headers, rows });
                *pos += 1; // consume End event
            }
            Event::Start(Tag::BlockQuote(_)) => {
                *pos += 1; // consume Start event
                let content = parse_blocks_until(events, pos, TagEnd::BlockQuote(None));
                elements.push(MdElement::BlockQuote(content));
                *pos += 1; // consume End event
            }
            Event::Rule => {
                elements.push(MdElement::ThematicBreak);
                *pos += 1;
            }
            Event::Html(_) => {
                let html = collect_html_block(events, pos);
                elements.push(MdElement::HtmlBlock(html));
            }
            _ => {
                *pos += 1; // skip unhandled events
            }
        }
    }
    
    elements
}

fn parse_blocks_until(events: &[Event], pos: &mut usize, end_tag: TagEnd) -> Vec<MdElement> {
    let mut elements = Vec::new();
    
    while *pos < events.len() {
        if let Event::End(ref tag_end) = events[*pos] {
            if std::mem::discriminant(tag_end) == std::mem::discriminant(&end_tag) {
                break;
            }
        }
        
        match &events[*pos] {
            Event::Start(Tag::Heading { level, .. }) => {
                *pos += 1;
                let content = parse_inlines(events, pos, TagEnd::Heading(*level));
                elements.push(MdElement::Heading {
                    level: *level as u8,
                    content,
                });
                *pos += 1;
            }
            Event::Start(Tag::Paragraph) => {
                *pos += 1;
                let content = parse_inlines(events, pos, TagEnd::Paragraph);
                elements.push(MdElement::Paragraph(content));
                *pos += 1;
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                *pos += 1;
                let language = if info.is_empty() {
                    None
                } else {
                    Some(info.to_string())
                };
                let mut code = String::new();
                while *pos < events.len() {
                    match &events[*pos] {
                        Event::Text(text) => {
                            code.push_str(text);
                            *pos += 1;
                        }
                        Event::End(TagEnd::CodeBlock) => break,
                        _ => *pos += 1,
                    }
                }
                elements.push(MdElement::CodeBlock { language, code });
                *pos += 1;
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Indented)) => {
                *pos += 1;
                let mut code = String::new();
                while *pos < events.len() {
                    match &events[*pos] {
                        Event::Text(text) => {
                            code.push_str(text);
                            *pos += 1;
                        }
                        Event::End(TagEnd::CodeBlock) => break,
                        _ => *pos += 1,
                    }
                }
                elements.push(MdElement::CodeBlock { 
                    language: None, 
                    code 
                });
                *pos += 1;
            }
            Event::Start(Tag::List(start_number)) => {
                *pos += 1;
                let items = parse_list_items(events, pos);
                let ordered = start_number.is_some();
                elements.push(MdElement::List {
                    ordered,
                    start: *start_number,
                    items,
                });
                *pos += 1;
            }
            Event::Start(Tag::BlockQuote(_)) => {
                *pos += 1;
                let content = parse_blocks_until(events, pos, TagEnd::BlockQuote(None));
                elements.push(MdElement::BlockQuote(content));
                *pos += 1;
            }
            Event::Rule => {
                elements.push(MdElement::ThematicBreak);
                *pos += 1;
            }
            Event::Html(_) => {
                let html = collect_html_block(events, pos);
                elements.push(MdElement::HtmlBlock(html));
            }
            // Bare inline events (tight lists emit text without Paragraph wrapper)
            Event::Text(_) | Event::Code(_) | Event::SoftBreak | Event::HardBreak
            | Event::Start(Tag::Strong) | Event::Start(Tag::Emphasis)
            | Event::Start(Tag::Strikethrough) | Event::Start(Tag::Link { .. })
            | Event::Start(Tag::Image { .. }) | Event::InlineHtml(_) => {
                let content = collect_bare_inlines(events, pos, &end_tag);
                if !content.is_empty() {
                    elements.push(MdElement::Paragraph(content));
                }
            }
            _ => {
                *pos += 1;
            }
        }
    }
    
    elements
}

/// Collect bare inline events into InlineElements (for tight list items).
/// Stops at block-level events or the container's end tag.
fn collect_bare_inlines(events: &[Event], pos: &mut usize, container_end: &TagEnd) -> Vec<InlineElement> {
    let mut inlines = Vec::new();

    while *pos < events.len() {
        // Stop at container end
        if let Event::End(ref tag_end) = events[*pos] {
            if std::mem::discriminant(tag_end) == std::mem::discriminant(container_end) {
                break;
            }
        }

        match &events[*pos] {
            // Stop at block-level start events
            Event::Start(Tag::Paragraph) | Event::Start(Tag::Heading { .. })
            | Event::Start(Tag::CodeBlock(_)) | Event::Start(Tag::List(_))
            | Event::Start(Tag::Table(_)) | Event::Start(Tag::BlockQuote(_)) => break,
            Event::Rule => break,

            Event::Text(text) => {
                inlines.push(InlineElement::Text(text.to_string()));
                *pos += 1;
            }
            Event::Code(code) => {
                inlines.push(InlineElement::Code(code.to_string()));
                *pos += 1;
            }
            Event::SoftBreak => {
                inlines.push(InlineElement::SoftBreak);
                *pos += 1;
            }
            Event::HardBreak => {
                inlines.push(InlineElement::HardBreak);
                *pos += 1;
            }
            Event::Start(Tag::Strong) => {
                *pos += 1;
                let content = parse_inlines(events, pos, TagEnd::Strong);
                inlines.push(InlineElement::Bold(content));
                *pos += 1;
            }
            Event::Start(Tag::Emphasis) => {
                *pos += 1;
                let content = parse_inlines(events, pos, TagEnd::Emphasis);
                inlines.push(InlineElement::Italic(content));
                *pos += 1;
            }
            Event::Start(Tag::Strikethrough) => {
                *pos += 1;
                let content = parse_inlines(events, pos, TagEnd::Strikethrough);
                inlines.push(InlineElement::Strikethrough(content));
                *pos += 1;
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                let url = dest_url.to_string();
                *pos += 1;
                let content = parse_inlines(events, pos, TagEnd::Link);
                inlines.push(InlineElement::Link { content, url });
                *pos += 1;
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                let url = dest_url.to_string();
                *pos += 1;
                let mut alt = String::new();
                while *pos < events.len() {
                    match &events[*pos] {
                        Event::Text(text) => {
                            alt.push_str(text);
                            *pos += 1;
                        }
                        Event::End(TagEnd::Image) => break,
                        _ => *pos += 1,
                    }
                }
                inlines.push(InlineElement::Image { alt, url });
                *pos += 1;
            }
            Event::InlineHtml(html) => {
                inlines.push(InlineElement::Html(html.to_string()));
                *pos += 1;
            }
            _ => {
                *pos += 1;
            }
        }
    }

    inlines
}

fn parse_inlines(events: &[Event], pos: &mut usize, end_tag: TagEnd) -> Vec<InlineElement> {
    let mut inlines = Vec::new();
    
    while *pos < events.len() {
        if let Event::End(ref tag_end) = events[*pos] {
            if std::mem::discriminant(tag_end) == std::mem::discriminant(&end_tag) {
                break;
            }
        }
        
        match &events[*pos] {
            Event::Text(text) => {
                inlines.push(InlineElement::Text(text.to_string()));
                *pos += 1;
            }
            Event::Code(code) => {
                inlines.push(InlineElement::Code(code.to_string()));
                *pos += 1;
            }
            Event::Start(Tag::Strong) => {
                *pos += 1;
                let content = parse_inlines(events, pos, TagEnd::Strong);
                inlines.push(InlineElement::Bold(content));
                *pos += 1;
            }
            Event::Start(Tag::Emphasis) => {
                *pos += 1;
                let content = parse_inlines(events, pos, TagEnd::Emphasis);
                inlines.push(InlineElement::Italic(content));
                *pos += 1;
            }
            Event::Start(Tag::Strikethrough) => {
                *pos += 1;
                let content = parse_inlines(events, pos, TagEnd::Strikethrough);
                inlines.push(InlineElement::Strikethrough(content));
                *pos += 1;
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                let url = dest_url.to_string();
                *pos += 1;
                let content = parse_inlines(events, pos, TagEnd::Link);
                inlines.push(InlineElement::Link { content, url });
                *pos += 1;
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                let url = dest_url.to_string();
                *pos += 1;
                // Collect alt text from inner text events
                let mut alt = String::new();
                while *pos < events.len() {
                    match &events[*pos] {
                        Event::Text(text) => {
                            alt.push_str(text);
                            *pos += 1;
                        }
                        Event::End(TagEnd::Image) => break,
                        _ => *pos += 1,
                    }
                }
                inlines.push(InlineElement::Image { alt, url });
                *pos += 1; // consume End event
            }
            Event::SoftBreak => {
                inlines.push(InlineElement::SoftBreak);
                *pos += 1;
            }
            Event::HardBreak => {
                inlines.push(InlineElement::HardBreak);
                *pos += 1;
            }
            Event::InlineHtml(html) => {
                inlines.push(InlineElement::Html(html.to_string()));
                *pos += 1;
            }
            _ => {
                *pos += 1; // skip unhandled events
            }
        }
    }
    
    inlines
}

fn parse_list_items(events: &[Event], pos: &mut usize) -> Vec<ListItem> {
    let mut items = Vec::new();
    
    while *pos < events.len() {
        match &events[*pos] {
            Event::Start(Tag::Item) => {
                *pos += 1;
                let mut checked = None;
                
                // Check for task list marker
                if *pos < events.len() {
                    if let Event::TaskListMarker(is_checked) = &events[*pos] {
                        checked = Some(*is_checked);
                        *pos += 1;
                    }
                }
                
                let content = parse_blocks_until(events, pos, TagEnd::Item);
                items.push(ListItem { checked, content });
                *pos += 1; // consume End event
            }
            Event::End(TagEnd::List(_)) => break,
            _ => *pos += 1,
        }
    }
    
    items
}

fn parse_table(events: &[Event], pos: &mut usize) -> (Vec<Vec<InlineElement>>, Vec<Vec<Vec<InlineElement>>>) {
    let mut headers = Vec::new();
    let mut rows = Vec::new();
    
    while *pos < events.len() {
        match &events[*pos] {
            Event::Start(Tag::TableHead) => {
                *pos += 1;
                headers = parse_table_row(events, pos, TagEnd::TableHead);
                *pos += 1; // consume End event
            }
            Event::Start(Tag::TableRow) => {
                *pos += 1;
                let row = parse_table_row(events, pos, TagEnd::TableRow);
                rows.push(row);
                *pos += 1; // consume End event
            }
            Event::End(TagEnd::Table) => break,
            _ => *pos += 1,
        }
    }
    
    (headers, rows)
}

fn parse_table_row(events: &[Event], pos: &mut usize, end_tag: TagEnd) -> Vec<Vec<InlineElement>> {
    let mut cells = Vec::new();
    
    while *pos < events.len() {
        if let Event::End(ref tag_end) = events[*pos] {
            if std::mem::discriminant(tag_end) == std::mem::discriminant(&end_tag) {
                break;
            }
        }
        
        match &events[*pos] {
            Event::Start(Tag::TableCell) => {
                *pos += 1;
                let cell_content = parse_inlines(events, pos, TagEnd::TableCell);
                cells.push(cell_content);
                *pos += 1; // consume End event
            }
            _ => *pos += 1,
        }
    }
    
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_heading() {
        let elements = parse_markdown("# Hello World");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::Heading { level, content } => {
                assert_eq!(*level, 1);
                assert_eq!(content.len(), 1);
                assert!(matches!(&content[0], InlineElement::Text(t) if t == "Hello World"));
            }
            _ => panic!("Expected Heading"),
        }
    }

    #[test]
    fn test_parse_paragraph_with_bold() {
        let elements = parse_markdown("This is **bold** text.");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::Paragraph(inlines) => {
                assert_eq!(inlines.len(), 3);
                assert!(matches!(&inlines[0], InlineElement::Text(t) if t == "This is "));
                assert!(matches!(&inlines[1], InlineElement::Bold(_)));
                assert!(matches!(&inlines[2], InlineElement::Text(t) if t == " text."));
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    #[test]
    fn test_parse_code_block() {
        let elements = parse_markdown("```rust\nfn main() {}\n```");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::CodeBlock { language, code } => {
                assert_eq!(language.as_deref(), Some("rust"));
                assert_eq!(code, "fn main() {}\n");
            }
            _ => panic!("Expected CodeBlock"),
        }
    }

    #[test]
    fn test_parse_unordered_list() {
        let elements = parse_markdown("- Item 1\n- Item 2\n- Item 3");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::List { ordered, items, .. } => {
                assert!(!ordered);
                assert_eq!(items.len(), 3);
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_parse_ordered_list() {
        let elements = parse_markdown("1. First\n2. Second");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::List { ordered, items, start } => {
                assert!(ordered);
                assert_eq!(items.len(), 2);
                assert_eq!(*start, Some(1));
            }
            _ => panic!("Expected ordered List"),
        }
    }

    #[test]
    fn test_parse_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |";
        let elements = parse_markdown(md);
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::Table { headers, rows } => {
                assert_eq!(headers.len(), 2);
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].len(), 2);
            }
            _ => panic!("Expected Table"),
        }
    }

    #[test]
    fn test_parse_link() {
        let elements = parse_markdown("[click here](https://example.com)");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::Paragraph(inlines) => {
                assert_eq!(inlines.len(), 1);
                match &inlines[0] {
                    InlineElement::Link { url, content } => {
                        assert_eq!(url, "https://example.com");
                        assert!(matches!(&content[0], InlineElement::Text(t) if t == "click here"));
                    }
                    _ => panic!("Expected Link"),
                }
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    #[test]
    fn test_parse_task_list() {
        let md = "- [x] Done\n- [ ] Todo";
        let elements = parse_markdown(md);
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::List { items, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].checked, Some(true));
                assert_eq!(items[1].checked, Some(false));
            }
            _ => panic!("Expected List with task items"),
        }
    }

    #[test]
    fn test_parse_blockquote() {
        let elements = parse_markdown("> This is a quote");
        assert_eq!(elements.len(), 1);
        assert!(matches!(&elements[0], MdElement::BlockQuote(_)));
    }

    #[test]
    fn test_parse_thematic_break() {
        let elements = parse_markdown("---");
        assert_eq!(elements.len(), 1);
        assert!(matches!(&elements[0], MdElement::ThematicBreak));
    }

    #[test]
    fn test_parse_strikethrough() {
        let elements = parse_markdown("~~deleted~~");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::Paragraph(inlines) => {
                assert!(matches!(&inlines[0], InlineElement::Strikethrough(_)));
            }
            _ => panic!("Expected Paragraph with Strikethrough"),
        }
    }

    #[test]
    fn test_tight_list_items_have_content() {
        // Tight lists (no blank lines between items) should preserve text content
        let elements = parse_markdown("- Item 1\n- Item 2\n- Item 3");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::List { items, .. } => {
                assert_eq!(items.len(), 3);
                // Each item MUST have content — this is the critical check
                for (i, item) in items.iter().enumerate() {
                    assert!(
                        !item.content.is_empty(),
                        "List item {i} has no content — tight list text was dropped"
                    );
                    // Verify text is actually there
                    let text = extract_text_from_elements(&item.content);
                    let expected = format!("Item {}", i + 1);
                    assert!(
                        text.contains(&expected),
                        "List item {i} content should contain '{expected}', got: '{text}'"
                    );
                }
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_tight_list_with_formatting() {
        // Tight list with bold/italic — must preserve formatted text
        let md = "- **Bold item**\n- *Italic item*\n- `code item`";
        let elements = parse_markdown(md);
        match &elements[0] {
            MdElement::List { items, .. } => {
                for (i, item) in items.iter().enumerate() {
                    assert!(
                        !item.content.is_empty(),
                        "Formatted list item {i} has no content"
                    );
                }
                let text0 = extract_text_from_elements(&items[0].content);
                assert!(text0.contains("Bold item"), "Bold item text missing: '{text0}'");
                let text1 = extract_text_from_elements(&items[1].content);
                assert!(text1.contains("Italic item"), "Italic item text missing: '{text1}'");
                let text2 = extract_text_from_elements(&items[2].content);
                assert!(text2.contains("code item"), "Code item text missing: '{text2}'");
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_loose_list_items_have_content() {
        // Loose lists (blank lines between items) should also work
        let md = "- Item A\n\n- Item B\n\n- Item C";
        let elements = parse_markdown(md);
        match &elements[0] {
            MdElement::List { items, .. } => {
                for (i, item) in items.iter().enumerate() {
                    assert!(
                        !item.content.is_empty(),
                        "Loose list item {i} has no content"
                    );
                }
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_task_list_item_text() {
        let md = "- [x] Done task\n- [ ] Todo task";
        let elements = parse_markdown(md);
        match &elements[0] {
            MdElement::List { items, .. } => {
                let text0 = extract_text_from_elements(&items[0].content);
                assert!(text0.contains("Done task"), "Task text missing: '{text0}'");
                let text1 = extract_text_from_elements(&items[1].content);
                assert!(text1.contains("Todo task"), "Task text missing: '{text1}'");
            }
            _ => panic!("Expected List"),
        }
    }

    /// Helper: extract all text recursively from a vec of MdElements
    fn extract_text_from_elements(elements: &[MdElement]) -> String {
        let mut s = String::new();
        for el in elements {
            match el {
                MdElement::Paragraph(inlines) | MdElement::Heading { content: inlines, .. } => {
                    s.push_str(&extract_text_from_inlines(inlines));
                }
                MdElement::BlockQuote(children) => {
                    s.push_str(&extract_text_from_elements(children));
                }
                MdElement::List { items, .. } => {
                    for item in items {
                        s.push_str(&extract_text_from_elements(&item.content));
                    }
                }
                MdElement::CodeBlock { code, .. } => s.push_str(code),
                MdElement::ThematicBreak | MdElement::Table { .. } => {}
                MdElement::HtmlBlock(html) => s.push_str(html),
            }
        }
        s
    }

    fn extract_text_from_inlines(inlines: &[InlineElement]) -> String {
        let mut s = String::new();
        for inline in inlines {
            match inline {
                InlineElement::Text(t) => s.push_str(t),
                InlineElement::Code(c) => s.push_str(c),
                InlineElement::Bold(c) | InlineElement::Italic(c) | InlineElement::Strikethrough(c) => {
                    s.push_str(&extract_text_from_inlines(c));
                }
                InlineElement::Link { content, .. } => {
                    s.push_str(&extract_text_from_inlines(content));
                }
                InlineElement::SoftBreak => s.push(' '),
                InlineElement::HardBreak => s.push('\n'),
                InlineElement::Image { alt, .. } => s.push_str(alt),
                InlineElement::Html(html) => s.push_str(html),
            }
        }
        s
    }

    #[test]
    fn test_parse_multiple_headings() {
        let md = "# H1\n\n## H2\n\n### H3";
        let elements = parse_markdown(md);
        assert_eq!(elements.len(), 3);
        for (i, expected_level) in [(0, 1), (1, 2), (2, 3)] {
            match &elements[i] {
                MdElement::Heading { level, .. } => assert_eq!(*level, expected_level),
                _ => panic!("Expected Heading at index {i}"),
            }
        }
    }

    // ---- HTML Parsing ----

    #[test]
    fn test_parse_html_block() {
        let elements = parse_markdown("<div>hello</div>\n");
        assert!(!elements.is_empty(), "Expected at least one element");
        let found = elements.iter().any(|el| {
            matches!(el, MdElement::HtmlBlock(s) if s.contains("<div>hello</div>"))
        });
        assert!(found, "Expected HtmlBlock containing '<div>hello</div>', got: {elements:?}");
    }

    #[test]
    fn test_parse_consecutive_html_lines_as_single_block() {
        let elements = parse_markdown("<div>\n<p>Hello</p>\n</div>\n");
        assert_eq!(elements.len(), 1);
        assert_eq!(
            elements[0],
            MdElement::HtmlBlock("<div>\n<p>Hello</p>\n</div>\n".to_string())
        );
    }

    #[test]
    fn test_parse_inline_html() {
        let elements = parse_markdown("text <b>bold</b> more");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::Paragraph(inlines) => {
                let html_count = inlines.iter().filter(|i| matches!(i, InlineElement::Html(_))).count();
                assert!(html_count >= 2, "Expected at least 2 Html inlines (<b> and </b>), got {html_count}: {inlines:?}");
                let has_bold_open = inlines.iter().any(|i| matches!(i, InlineElement::Html(s) if s.contains("<b>")));
                assert!(has_bold_open, "Expected Html inline with '<b>', got: {inlines:?}");
                let has_bold_close = inlines.iter().any(|i| matches!(i, InlineElement::Html(s) if s.contains("</b>")));
                assert!(has_bold_close, "Expected Html inline with '</b>', got: {inlines:?}");
            }
            _ => panic!("Expected Paragraph, got: {elements:?}"),
        }
    }

    #[test]
    fn test_parse_html_br_tag() {
        let elements = parse_markdown("line1<br>line2");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::Paragraph(inlines) => {
                let has_br = inlines.iter().any(|i| {
                    matches!(i, InlineElement::Html(s) if s.contains("<br>"))
                });
                assert!(has_br, "Expected Html inline containing '<br>', got: {inlines:?}");
            }
            _ => panic!("Expected Paragraph, got: {elements:?}"),
        }
    }

    #[test]
    fn test_parse_html_hr_tag() {
        let elements = parse_markdown("<hr>\n");
        assert!(!elements.is_empty(), "Expected at least one element");
        let found = elements.iter().any(|el| {
            matches!(el, MdElement::HtmlBlock(s) if s.contains("<hr>"))
        });
        assert!(found, "Expected HtmlBlock containing '<hr>', got: {elements:?}");
    }

    // ---- Nested Structures ----

    #[test]
    fn test_parse_nested_list() {
        let md = "- Item 1\n  - Sub 1\n  - Sub 2\n- Item 2";
        let elements = parse_markdown(md);
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::List { ordered, items, .. } => {
                assert!(!ordered);
                assert_eq!(items.len(), 2);
                let has_inner_list = items[0].content.iter().any(|el| matches!(el, MdElement::List { .. }));
                assert!(has_inner_list, "Expected nested List in first item, got: {:?}", items[0].content);
            }
            _ => panic!("Expected List, got: {elements:?}"),
        }
    }

    #[test]
    fn test_parse_nested_blockquote() {
        let elements = parse_markdown("> > nested");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::BlockQuote(inner) => {
                assert!(!inner.is_empty(), "Outer BlockQuote should not be empty");
                let has_inner_bq = inner.iter().any(|el| matches!(el, MdElement::BlockQuote(_)));
                assert!(has_inner_bq, "Expected nested BlockQuote, got: {inner:?}");
            }
            _ => panic!("Expected BlockQuote, got: {elements:?}"),
        }
    }

    #[test]
    fn test_parse_list_with_link() {
        let elements = parse_markdown("- [link text](https://example.com)");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::List { items, .. } => {
                assert_eq!(items.len(), 1);
                let has_link = items[0].content.iter().any(|el| {
                    if let MdElement::Paragraph(inlines) = el {
                        inlines.iter().any(|i| matches!(i, InlineElement::Link { url, .. } if url == "https://example.com"))
                    } else {
                        false
                    }
                });
                assert!(has_link, "Expected Link inline in list item, got: {:?}", items[0].content);
            }
            _ => panic!("Expected List, got: {elements:?}"),
        }
    }

    #[test]
    fn test_parse_list_with_code() {
        let elements = parse_markdown("- some `code` text");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::List { items, .. } => {
                assert_eq!(items.len(), 1);
                let inlines: Vec<&InlineElement> = items[0].content.iter().flat_map(|el| {
                    if let MdElement::Paragraph(inlines) = el { inlines.iter().collect::<Vec<_>>() } else { vec![] }
                }).collect();
                let has_text = inlines.iter().any(|i| matches!(i, InlineElement::Text(_)));
                let has_code = inlines.iter().any(|i| matches!(i, InlineElement::Code(c) if c == "code"));
                assert!(has_text, "Expected Text inlines, got: {inlines:?}");
                assert!(has_code, "Expected Code inline with 'code', got: {inlines:?}");
            }
            _ => panic!("Expected List, got: {elements:?}"),
        }
    }

    #[test]
    fn test_parse_list_with_bold_and_italic() {
        let elements = parse_markdown("- **bold** and *italic*");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::List { items, .. } => {
                assert_eq!(items.len(), 1);
                let inlines: Vec<&InlineElement> = items[0].content.iter().flat_map(|el| {
                    if let MdElement::Paragraph(inlines) = el { inlines.iter().collect::<Vec<_>>() } else { vec![] }
                }).collect();
                let has_bold = inlines.iter().any(|i| matches!(i, InlineElement::Bold(_)));
                let has_italic = inlines.iter().any(|i| matches!(i, InlineElement::Italic(_)));
                assert!(has_bold, "Expected Bold inline, got: {inlines:?}");
                assert!(has_italic, "Expected Italic inline, got: {inlines:?}");
            }
            _ => panic!("Expected List, got: {elements:?}"),
        }
    }

    #[test]
    fn test_parse_heading_with_link() {
        let elements = parse_markdown("# [Click here](https://example.com)");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::Heading { level, content } => {
                assert_eq!(*level, 1);
                let has_link = content.iter().any(|i| {
                    matches!(i, InlineElement::Link { url, .. } if url == "https://example.com")
                });
                assert!(has_link, "Expected Link in heading content, got: {content:?}");
            }
            _ => panic!("Expected Heading, got: {elements:?}"),
        }
    }

    // ---- Complex GFM ----

    #[test]
    fn test_parse_table_multiple_rows() {
        let md = "| Name | Age | City |\n|------|-----|------|\n| Alice | 30 | NYC |\n| Bob | 25 | LA |\n| Carol | 35 | SF |";
        let elements = parse_markdown(md);
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::Table { headers, rows } => {
                assert_eq!(headers.len(), 3, "Expected 3 header columns");
                assert_eq!(rows.len(), 3, "Expected 3 data rows");
                let h0_text = extract_text_from_inlines(&headers[0]);
                assert!(h0_text.contains("Name"), "First header should be 'Name', got: '{h0_text}'");
                let cell_text = extract_text_from_inlines(&rows[0][0]);
                assert!(cell_text.contains("Alice"), "First cell should contain 'Alice', got: '{cell_text}'");
            }
            _ => panic!("Expected Table, got: {elements:?}"),
        }
    }

    #[test]
    fn test_parse_code_block_with_language() {
        let elements = parse_markdown("```rust\nfn main() {}\n```");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::CodeBlock { language, code } => {
                assert_eq!(language.as_deref(), Some("rust"), "Expected language 'rust'");
                assert!(code.contains("fn main()"), "Expected code containing 'fn main()', got: '{code}'");
            }
            _ => panic!("Expected CodeBlock, got: {elements:?}"),
        }
    }

    #[test]
    fn test_parse_code_block_no_language() {
        let elements = parse_markdown("```\nsome code here\n```");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::CodeBlock { language, code } => {
                assert_eq!(language, &None, "Expected no language");
                assert!(code.contains("some code here"), "Expected code content, got: '{code}'");
            }
            _ => panic!("Expected CodeBlock, got: {elements:?}"),
        }
    }

    #[test]
    fn test_parse_image() {
        let elements = parse_markdown("![alt text](path.png)");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::Paragraph(inlines) => {
                assert_eq!(inlines.len(), 1);
                match &inlines[0] {
                    InlineElement::Image { alt, url } => {
                        assert_eq!(alt, "alt text");
                        assert_eq!(url, "path.png");
                    }
                    _ => panic!("Expected Image inline, got: {inlines:?}"),
                }
            }
            _ => panic!("Expected Paragraph, got: {elements:?}"),
        }
    }

    #[test]
    fn test_parse_soft_break() {
        let elements = parse_markdown("line1\nline2");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::Paragraph(inlines) => {
                let has_soft_break = inlines.iter().any(|i| matches!(i, InlineElement::SoftBreak));
                assert!(has_soft_break, "Expected SoftBreak between lines, got: {inlines:?}");
                assert!(inlines.len() >= 3, "Expected at least Text, SoftBreak, Text; got: {inlines:?}");
            }
            _ => panic!("Expected Paragraph, got: {elements:?}"),
        }
    }

    #[test]
    fn test_parse_hard_break() {
        let elements = parse_markdown("line1  \nline2");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::Paragraph(inlines) => {
                let has_hard_break = inlines.iter().any(|i| matches!(i, InlineElement::HardBreak));
                assert!(has_hard_break, "Expected HardBreak (from trailing spaces), got: {inlines:?}");
            }
            _ => panic!("Expected Paragraph, got: {elements:?}"),
        }
    }

    // ---- Edge Cases ----

    #[test]
    fn test_parse_empty_input() {
        let elements = parse_markdown("");
        assert!(elements.is_empty(), "Expected empty Vec for empty input, got: {elements:?}");
    }

    #[test]
    fn test_parse_only_whitespace() {
        let elements = parse_markdown("   \n\n  ");
        assert!(elements.is_empty(), "Expected empty Vec for whitespace-only input, got: {elements:?}");
    }

    #[test]
    fn test_parse_consecutive_paragraphs() {
        let elements = parse_markdown("First paragraph.\n\nSecond paragraph.");
        assert_eq!(elements.len(), 2, "Expected two paragraphs separated by blank line");
        assert!(matches!(&elements[0], MdElement::Paragraph(_)), "First element should be Paragraph");
        assert!(matches!(&elements[1], MdElement::Paragraph(_)), "Second element should be Paragraph");
        match &elements[0] {
            MdElement::Paragraph(inlines) => {
                let text = extract_text_from_inlines(inlines);
                assert!(text.contains("First paragraph"), "First paragraph text mismatch: '{text}'");
            }
            _ => unreachable!(),
        }
        match &elements[1] {
            MdElement::Paragraph(inlines) => {
                let text = extract_text_from_inlines(inlines);
                assert!(text.contains("Second paragraph"), "Second paragraph text mismatch: '{text}'");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_parse_task_list_checked_and_unchecked() {
        let md = "- [x] Task A\n- [ ] Task B\n- [x] Task C\n- [ ] Task D";
        let elements = parse_markdown(md);
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::List { items, .. } => {
                assert_eq!(items.len(), 4);
                assert_eq!(items[0].checked, Some(true), "Task A should be checked");
                assert_eq!(items[1].checked, Some(false), "Task B should be unchecked");
                assert_eq!(items[2].checked, Some(true), "Task C should be checked");
                assert_eq!(items[3].checked, Some(false), "Task D should be unchecked");
                let text_a = extract_text_from_elements(&items[0].content);
                assert!(text_a.contains("Task A"), "Expected 'Task A', got: '{text_a}'");
                let text_d = extract_text_from_elements(&items[3].content);
                assert!(text_d.contains("Task D"), "Expected 'Task D', got: '{text_d}'");
            }
            _ => panic!("Expected List, got: {elements:?}"),
        }
    }

    // ---- Combined Elements ----

    #[test]
    fn test_parse_bold_inside_link() {
        let elements = parse_markdown("[**bold link**](https://example.com)");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::Paragraph(inlines) => {
                assert_eq!(inlines.len(), 1);
                match &inlines[0] {
                    InlineElement::Link { url, content } => {
                        assert_eq!(url, "https://example.com");
                        let has_bold = content.iter().any(|i| matches!(i, InlineElement::Bold(_)));
                        assert!(has_bold, "Expected Bold inside Link, got: {content:?}");
                    }
                    _ => panic!("Expected Link inline, got: {inlines:?}"),
                }
            }
            _ => panic!("Expected Paragraph, got: {elements:?}"),
        }
    }

    #[test]
    fn test_parse_italic_inside_bold() {
        let elements = parse_markdown("***bold italic***");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            MdElement::Paragraph(inlines) => {
                assert_eq!(inlines.len(), 1);
                let text = extract_text_from_inlines(inlines);
                assert!(text.contains("bold italic"), "Expected 'bold italic' text, got: '{text}'");
                // pulldown-cmark may produce Italic(Bold(text)) or Bold(Italic(text))
                let has_nested = match &inlines[0] {
                    InlineElement::Italic(inner) => inner.iter().any(|i| matches!(i, InlineElement::Bold(_))),
                    InlineElement::Bold(inner) => inner.iter().any(|i| matches!(i, InlineElement::Italic(_))),
                    _ => false,
                };
                assert!(has_nested, "Expected Bold inside Italic or Italic inside Bold, got: {inlines:?}");
            }
            _ => panic!("Expected Paragraph, got: {elements:?}"),
        }
    }

    #[test]
    fn test_parse_complex_document() {
        let md = "# Title\n\nA paragraph of text.\n\n- Item 1\n- Item 2\n\n```python\nprint('hello')\n```";
        let elements = parse_markdown(md);
        assert!(elements.len() >= 4, "Expected at least 4 elements (heading, paragraph, list, code block), got {}", elements.len());
        assert!(matches!(&elements[0], MdElement::Heading { level: 1, .. }), "First element should be H1 heading");
        assert!(matches!(&elements[1], MdElement::Paragraph(_)), "Second element should be Paragraph");
        assert!(matches!(&elements[2], MdElement::List { .. }), "Third element should be List");
        assert!(matches!(&elements[3], MdElement::CodeBlock { .. }), "Fourth element should be CodeBlock");
        match &elements[3] {
            MdElement::CodeBlock { language, code } => {
                assert_eq!(language.as_deref(), Some("python"));
                assert!(code.contains("print('hello')"), "Code should contain print statement");
            }
            _ => unreachable!(),
        }
    }
}
