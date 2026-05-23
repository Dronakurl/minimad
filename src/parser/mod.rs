mod line_parser;
mod options;
mod text_parser;

pub use {line_parser::*, options::*, text_parser::*};

#[test]
fn indented_code_between_fences() {
    use crate::*;
    let md = r#"
        outside
        ```code
        a
            b
        ```
    "#;
    assert_eq!(
        parse_text(md, Options::default().clean_indentations(true)),
        Text {
            lines: vec![
                Line::new_paragraph(vec![Compound::raw_str("outside")]),
                Line::new_code(Compound::raw_str("a")),
                Line::new_code(Compound::raw_str("    b")),
            ]
        },
    );
}

#[test]
fn test_clean() {
    use crate::*;
    let text = r#"
        bla bla bla
        * item 1
        * item 2
    "#;
    assert_eq!(
        parse_text(
            text,
            Options {
                clean_indentations: true,
                ..Default::default()
            }
        ),
        Text {
            lines: vec![
                Line::from("bla bla bla"),
                Line::from("* item 1"),
                Line::from("* item 2"),
            ]
        },
    );
}

#[test]
fn test_inline_code_continuation() {
    use crate::*;
    let md = r#"
        bla bla `code
        again` bla
    "#;
    // Without continuation
    let options = Options::default().clean_indentations(true);
    assert_eq!(
        parse_text(md, options),
        Text {
            lines: vec![Line::from("bla bla `code"), Line::from("again` bla"),]
        },
    );
    // With continuation
    let options = Options::default()
        .clean_indentations(true)
        .continue_inline_code(true);
    assert_eq!(
        parse_text(md, options),
        Text {
            lines: vec![Line::from("bla bla `code`"), Line::from("`again` bla"),]
        },
    );
}

// CommonMark compliance tests for list indentation
#[test]
fn commonmark_basic_list_nesting() {
    use crate::*;
    // Basic nesting with 2 spaces
    // Note: 4 spaces before marker creates a code block, not a list item
    let md = r#"* level 0
  * level 1
   * level 2"#;
    let text = parse_text(md, Options::default());
    assert_eq!(text.lines.len(), 3);

    // Check that level 1 is nested under level 0
    if let Line::Normal(composite) = &text.lines[0] {
        assert_eq!(composite.style, CompositeStyle::ListItem(0));
    } else {
        panic!("First line should be a list item");
    }

    if let Line::Normal(composite) = &text.lines[1] {
        assert_eq!(composite.style, CompositeStyle::ListItem(1));
    } else {
        panic!("Second line should be a list item");
    }

    if let Line::Normal(composite) = &text.lines[2] {
        // 3 spaces before *: marker column=3
        // Parent line 1 has column=2, width=2, so edge at 4
        // 3 >= 2 and 3 < 4, so it's in the same nested list as line 1
        // Both line 1 and line 2 are nested under line 0, so both at depth 1
        assert_eq!(composite.style, CompositeStyle::ListItem(1));
    } else {
        panic!("Third line should be a list item, got {:?}", text.lines[2]);
    }
}

#[test]
fn commonmark_list_nesting_with_different_indents() {
    use crate::*;
    // According to CommonMark, 1 space is NOT enough for nesting
    // 2 and 3 spaces ARE enough
    let md = r#"* level 0
 * not nested (1 space)
  * nested (2 spaces)
   * nested (3 spaces)"#;
    let text = parse_text(md, Options::default());
    assert_eq!(text.lines.len(), 4);

    // First line: depth 0
    if let Line::Normal(composite) = &text.lines[0] {
        assert_eq!(composite.style, CompositeStyle::ListItem(0));
    } else {
        panic!("Line 0 should be a list item");
    }

    // Second line: 1 space before *, column=1, parent column=0, parent width=2
    // 1 >= 0+2 is false, so NOT nested, depth 0
    if let Line::Normal(composite) = &text.lines[1] {
        assert_eq!(
            composite.style,
            CompositeStyle::ListItem(0),
            "Line 1 with 1 space should NOT be nested, got {:?}",
            composite.style
        );
    } else {
        panic!("Line 1 should be a list item");
    }

    // Third line: 2 spaces before *, column=2, parent column=0, parent width=2
    // 2 >= 0+2 is true, so nested at depth 1
    if let Line::Normal(composite) = &text.lines[2] {
        assert_eq!(
            composite.style,
            CompositeStyle::ListItem(1),
            "Line 2 with 2 spaces should be nested, got {:?}",
            composite.style
        );
    } else {
        panic!("Line 2 should be a list item");
    }

    // Fourth line: 3 spaces before *, column=3, parent column=0, parent width=2
    // 3 >= 0+2 is true, so nested at depth 1
    if let Line::Normal(composite) = &text.lines[3] {
        assert_eq!(
            composite.style,
            CompositeStyle::ListItem(1),
            "Line 3 with 3 spaces should be nested, got {:?}",
            composite.style
        );
    } else {
        panic!("Line 3 should be a list item");
    }
}

#[test]
fn commonmark_list_no_nesting_with_4_spaces() {
    use crate::*;
    // 4 spaces before marker should NOT create nesting (it's a code block)
    let md = r#"* level 0
    * not nested"#;
    let text = parse_text(md, Options::default());
    assert_eq!(text.lines.len(), 2);

    // First line is list item at depth 0
    if let Line::Normal(composite) = &text.lines[0] {
        assert_eq!(composite.style, CompositeStyle::ListItem(0));
    } else {
        panic!("First line should be a list item");
    }

    // Second line should be code (4 spaces), not a list item
    if let Line::Normal(composite) = &text.lines[1] {
        assert_eq!(composite.style, CompositeStyle::Code);
    } else {
        panic!("Second line should be code");
    }
}

#[test]
fn commonmark_ordered_list_nesting() {
    use crate::*;
    // Test with ordered list markers of different widths
    let md = r#"1. level 0
   2. level 1"#;
    let text = parse_text(md, Options::default());
    assert_eq!(text.lines.len(), 2);

    // Both should be list items
    if let Line::Normal(composite) = &text.lines[0] {
        assert_eq!(composite.style, CompositeStyle::ListItem(0));
    }

    if let Line::Normal(composite) = &text.lines[1] {
        // "   2. " has column=3, parent has marker "1. " at column=0 with width=3
        // 3 >= 0 + 3, so it should be nested at depth 1
        assert_eq!(composite.style, CompositeStyle::ListItem(1));
    }
}

#[test]
fn commonmark_list_same_level() {
    use crate::*;
    // Test that same indentation creates same-level items
    let md = r#"* item 1
* item 2
* item 3"#;
    let text = parse_text(md, Options::default());
    assert_eq!(text.lines.len(), 3);

    for (i, line) in text.lines.iter().enumerate() {
        if let Line::Normal(composite) = line {
            assert_eq!(
                composite.style,
                CompositeStyle::ListItem(0),
                "Line {} should be at depth 0",
                i
            );
        } else {
            panic!("Line {} should be a list item", i);
        }
    }
}

#[test]
fn commonmark_list_mixed_markers() {
    use crate::*;
    // Test that different bullet markers can nest
    let md = r#"- parent
  * child"#;
    let text = parse_text(md, Options::default());
    assert_eq!(text.lines.len(), 2);

    if let Line::Normal(composite) = &text.lines[0] {
        assert_eq!(composite.style, CompositeStyle::ListItem(0));
    }

    if let Line::Normal(composite) = &text.lines[1] {
        // "* " at column=2, parent "- " at column=0 with width=2
        // 2 >= 0 + 2, so it should be nested at depth 1
        assert_eq!(composite.style, CompositeStyle::ListItem(1));
    }
}

#[test]
fn commonmark_tab_indented_list() {
    use crate::*;
    // Test tab-indented lists (CommonMark: tab at start followed by list marker)
    let md = "- Einbruchsversuch\n\t- Tür von der Garage ins Haus. Muss die immer offen bleiben.\n\t- Fahrrad gestohlen\n\t- Lichtschacht vorne am Eingang. Vergittern\n- Mäuse";

    let text = parse_text(md, Options::default());
    assert_eq!(text.lines.len(), 5);

    // First line: depth 0
    if let Line::Normal(composite) = &text.lines[0] {
        assert_eq!(composite.style, CompositeStyle::ListItem(0));
    } else {
        panic!("Line 0 should be a list item");
    }

    // Second line: should be nested (depth 1) because of tab
    // Tab is treated as 1 character, but for CommonMark it should be enough
    if let Line::Normal(composite) = &text.lines[1] {
        assert_eq!(
            composite.style,
            CompositeStyle::ListItem(1),
            "Tab-indented line should be nested at depth 1, got {:?}",
            composite.style
        );
    } else {
        panic!("Line 1 should be a list item");
    }

    // Third line: also nested
    if let Line::Normal(composite) = &text.lines[2] {
        assert_eq!(composite.style, CompositeStyle::ListItem(1));
    }

    // Fourth line: also nested
    if let Line::Normal(composite) = &text.lines[3] {
        assert_eq!(composite.style, CompositeStyle::ListItem(1));
    }

    // Fifth line: back to depth 0
    if let Line::Normal(composite) = &text.lines[4] {
        assert_eq!(composite.style, CompositeStyle::ListItem(0));
    }
}

#[test]
fn commonmark_tab_indented_list_with_different_markers() {
    use crate::*;
    // Test the exact case from the user's note
    let md = "- Einbruchsversuch\n\t- Tür von der Garage ins Haus. Muss die immer offen bleiben.\n\t- Fahrrad gestohlen\n\t- Lichtschacht vorne am Eingang. Vergittern";

    let text = parse_text(md, Options::default());
    assert_eq!(text.lines.len(), 4);

    // All lines should have the correct depth and complete text
    if let Line::Normal(composite) = &text.lines[0] {
        assert_eq!(composite.style, CompositeStyle::ListItem(0));
        assert_eq!(composite.compounds[0].src, "Einbruchsversuch");
    } else {
        panic!("Line 0 should be a list item");
    }

    if let Line::Normal(composite) = &text.lines[1] {
        assert_eq!(composite.style, CompositeStyle::ListItem(1));
        assert_eq!(
            composite.compounds[0].src,
            "Tür von der Garage ins Haus. Muss die immer offen bleiben."
        );
    } else {
        panic!("Line 1 should be a nested list item");
    }

    if let Line::Normal(composite) = &text.lines[2] {
        assert_eq!(composite.style, CompositeStyle::ListItem(1));
        assert_eq!(composite.compounds[0].src, "Fahrrad gestohlen");
    } else {
        panic!("Line 2 should be a nested list item");
    }

    if let Line::Normal(composite) = &text.lines[3] {
        assert_eq!(composite.style, CompositeStyle::ListItem(1));
        assert_eq!(
            composite.compounds[0].src,
            "Lichtschacht vorne am Eingang. Vergittern"
        );
    } else {
        panic!("Line 3 should be a nested list item");
    }
}

#[test]
fn commonmark_nested_tab_list_example() {
    use crate::*;
    let md = "- some text\n\t- some text\n\t- some text\n\t- some text\n- some text";

    let text = parse_text(md, Options::default());
    assert_eq!(text.lines.len(), 5);

    if let Line::Normal(composite) = &text.lines[0] {
        assert_eq!(composite.style, CompositeStyle::ListItem(0));
    } else {
        panic!("Line 0 should be a list item");
    }

    for i in 1..=3 {
        if let Line::Normal(composite) = &text.lines[i] {
            assert_eq!(
                composite.style,
                CompositeStyle::ListItem(1),
                "Line {} should be nested",
                i
            );
        } else {
            panic!("Line {} should be a list item", i);
        }
    }

    if let Line::Normal(composite) = &text.lines[4] {
        assert_eq!(composite.style, CompositeStyle::ListItem(0));
    } else {
        panic!("Line 4 should be a list item");
    }
}

#[test]
fn commonmark_tab_indented_list_plus_marker() {
    use crate::*;
    // Test with plus markers
    let md = "+ parent\n\t+ child 1\n\t+ child 2";

    let text = parse_text(md, Options::default());
    assert_eq!(text.lines.len(), 3);

    if let Line::Normal(composite) = &text.lines[0] {
        assert_eq!(composite.style, CompositeStyle::ListItem(0));
        assert_eq!(composite.compounds[0].src, "parent");
    }

    if let Line::Normal(composite) = &text.lines[1] {
        assert_eq!(composite.style, CompositeStyle::ListItem(1));
        assert_eq!(composite.compounds[0].src, "child 1");
    }

    if let Line::Normal(composite) = &text.lines[2] {
        assert_eq!(composite.style, CompositeStyle::ListItem(1));
        assert_eq!(composite.compounds[0].src, "child 2");
    }
}

#[test]
fn commonmark_tab_indented_list_star_marker() {
    use crate::*;
    // Test with asterisk markers
    let md = "* parent\n\t* child 1\n\t* child 2";

    let text = parse_text(md, Options::default());
    assert_eq!(text.lines.len(), 3);

    if let Line::Normal(composite) = &text.lines[0] {
        assert_eq!(composite.style, CompositeStyle::ListItem(0));
        assert_eq!(composite.compounds[0].src, "parent");
    }

    if let Line::Normal(composite) = &text.lines[1] {
        assert_eq!(composite.style, CompositeStyle::ListItem(1));
        assert_eq!(composite.compounds[0].src, "child 1");
    }

    if let Line::Normal(composite) = &text.lines[2] {
        assert_eq!(composite.style, CompositeStyle::ListItem(1));
        assert_eq!(composite.compounds[0].src, "child 2");
    }
}

#[test]
fn commonmark_tab_indented_list_mixed_markers() {
    use crate::*;
    // Test with mixed markers
    let md = "- parent\n\t* child with star\n\t+ child with plus";

    let text = parse_text(md, Options::default());
    assert_eq!(text.lines.len(), 3);

    if let Line::Normal(composite) = &text.lines[0] {
        assert_eq!(composite.style, CompositeStyle::ListItem(0));
        assert_eq!(composite.compounds[0].src, "parent");
    }

    if let Line::Normal(composite) = &text.lines[1] {
        assert_eq!(composite.style, CompositeStyle::ListItem(1));
        assert_eq!(composite.compounds[0].src, "child with star");
    }

    if let Line::Normal(composite) = &text.lines[2] {
        assert_eq!(composite.style, CompositeStyle::ListItem(1));
        assert_eq!(composite.compounds[0].src, "child with plus");
    }
}
