# Comprehensive GitHub Flavored Markdown Test

This document exercises all GFM features for testing the Markdown viewer.

---

## Section 1: Headings

### Heading Level 3

#### Heading Level 4

##### Heading Level 5

###### Heading Level 6

---

## Section 2: Text Formatting

This paragraph demonstrates **bold text**, *italic text*, ***bold and italic text***, ~~strikethrough~~, and `inline code`.

You can also use __alternate bold__ and _alternate italic_ syntax, as well as ___alternate bold italic___.

Special characters like <, >, & and quotes should render correctly: "quoted" and 'single-quoted'.

---

## Section 3: Code Blocks

### Rust Example

```rust
fn main() {
    let message = "Hello, World!";
    println!("{}", message);
}
```

### Python Example

```python
def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n-1) + fibonacci(n-2)

result = fibonacci(10)
print(f"Result: {result}")
```

### JavaScript Example

```javascript
const greet = (name) => {
    console.log(`Hello, ${name}!`);
};

greet("World");
```

### Bash Example

```bash
#!/bin/bash
for i in {1..5}; do
    echo "Iteration $i"
done
```

### JSON Example

```json
{
    "name": "MarkdownViewer",
    "version": "1.0.0",
    "features": ["rendering", "navigation"],
    "active": true
}
```

### HTML Example

```html
<!DOCTYPE html>
<html>
<head>
    <title>Test Page</title>
</head>
<body>
    <h1>Welcome</h1>
    <p>This is a test.</p>
</body>
</html>
```

### CSS Example

```css
.container {
    display: flex;
    justify-content: center;
    gap: 1rem;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
}

.item {
    padding: 1rem;
    border-radius: 0.5rem;
    color: white;
}
```

---

## Section 4: Lists

### Unordered Lists (Nested)

- Item 1
- Item 2
  - Nested 2.1
  - Nested 2.2
    - Nested 2.2.1
    - Nested 2.2.2
  - Nested 2.3
- Item 3

### Ordered Lists (Nested)

1. First item
2. Second item
   1. Second.1
   2. Second.2
      1. Second.2.a
      2. Second.2.b
   3. Second.3
3. Third item

### Task Lists

- [x] Completed task
- [ ] Uncompleted task
- [x] Another completed task
  - [x] Nested completed subtask
  - [ ] Nested uncompleted subtask
- [ ] Task with `code` in description

---

## Section 5: Tables

### Simple Table

| Name    | Type      | Status  |
|---------|-----------|---------|
| Feature | String    | Active  |
| Bug     | Boolean   | Fixed   |
| Task    | Int       | Pending |

### Complex Table with Alignment

| Left Aligned | Center Aligned | Right Aligned |
|:-------------|:--------------:|---------------:|
| L1           |       C1       |              R1 |
| Left text    |    Centered    |        Right  |
| Another left |      More      |         More R |

### Table with Formatting

| Code | Bold | Emphasis |
|------|------|----------|
| `fn()` | **strong** | *emphasis* |
| `const x` | ***bold italic*** | ~~strikethrough~~ |

### Wide Table (tests column wrapping)

| Feature | Description | Notes |
|---------|-------------|-------|
| Column width auto-layout | Columns wrap their content when the table would otherwise exceed the viewport width, using a proportional distribution based on each column's preferred vs. minimum width. | This mirrors the CSS `table-layout: auto` algorithm used by browsers. |
| Overflow fallback | When even the longest word in a column exceeds the available space, the table is allowed to overflow horizontally rather than truncate mid-word. | Non-table paragraphs remain bounded to the viewport regardless. |
| Visual balance | Wider-content columns receive more space than narrow-content columns, so a long description does not force a short code column to be equally wide. | The slack between min and max width drives the weighting. |

### Table with `<br>` line breaks (uneven row heights)

| Field | Values | Notes |
|-------|--------|-------|
| Mode | 0x01: heating<br>0x02: cooling<br>0x03: venting<br>0x04: auto | Row heights should align; stripe backgrounds should cover the full row. |
| Fan  | 0x00: auto<br>0x02: 1<br>0x06: 2<br>0x03: 3<br>0x07: 4<br>0x05: 5 | Adjacent cells have vastly different line counts. |
| Temp | 16..31 °C | Short cell alongside tall ones. |

---

## Section 6: Blockquotes

> This is a simple blockquote.
> It can span multiple lines.

> This is a blockquote with formatting:
> - **bold text**
> - *italic text*
> - `code blocks`

> Level 1 blockquote
>> Level 2 nested blockquote
>>> Level 3 deeply nested blockquote
>>
>> Back to level 2
>
> Back to level 1

---

## Section 7: Links and Images

### External Links

- [GitHub](https://github.com)
- [Google Search](https://www.google.com)
- [Rust Official](https://www.rust-lang.org)

### Relative Links

- [Other Document](other.md)
- [Back to Top](#comprehensive-github-flavored-markdown-test)
- [Link to Section 5](#section-5-tables)

### Images

![Rust Logo](rust-logo.png)

---

## Section 8: Horizontal Rules

Here's a horizontal rule below:

---

Another one:

---

And another:

***

---

## Section 9: Nested Formatting

- Nested formatting inside **bold with *italic* inside**
- More: *italic with **bold** inside*
- Complex: ***all bold and italic with ~~strikethrough~~ inside***
- Code in bold: **`const x = 5`**

---

## Section 10: Unicode and Emoji

Status indicators:
- ✅ Completed
- ❌ Failed
- 🚀 In progress
- ⚠️ Warning
- 📝 Documentation
- 🔧 Configuration
- 🐛 Bug

---

## Section 11: Long Paragraph with Line Breaks

This is a long paragraph to test text wrapping and rendering. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.

Here's another paragraph with intentional line breaks  
for testing purposes. This should render as a new line  
within the same paragraph.

---

## Section 12: Inline HTML

You can include inline HTML like <span style="color: red;">red text</span> or <b>bold</b>.

<!-- This is an HTML comment and should be handled appropriately -->

---

## Section 13: Mixed Content

A paragraph with **bold**, *italic*, `code`, a [link](https://example.com), and special chars (<>&).

- List item with [link](other.md)
- Item with **bold and *italic***
- Item with `code and **bold**`

> Blockquote with [link](other.md) and **formatting**

---

## Conclusion

This comprehensive test document covers all major GitHub Flavored Markdown features. Use this to verify that the markdown viewer renders everything correctly.

[Navigate to other document](other.md)
