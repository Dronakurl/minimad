use crate::*;

/// Parse a markdown string into a text
pub fn parse(
    md: &str,
    options: Options,
) -> Text<'_> {
    if options.clean_indentations {
        parse_lines(clean::lines(md).into_iter(), options)
    } else {
        parse_lines(md.lines(), options)
    }
}

/// State for tracking container blocks during parsing
/// This enables CommonMark-compliant indentation handling
#[derive(Debug, Clone)]
struct ParseState {
    /// Stack of active list contexts (most recent first)
    list_stack: Vec<ListContext>,
    /// Stack of active blockquote contexts
    quote_stack: Vec<u32>,
    /// Are we currently in a code fence?
    between_fences: bool,
}

/// Context for an active list container
#[derive(Debug, Clone)]
pub struct ListContext {
    /// Column position (0-indexed) where the list marker starts
    pub marker_column: u32,
    /// Width of the list marker (e.g., 2 for "- ", 3 for "10. ")
    pub marker_width: u32,
    /// Current depth of this list
    pub depth: u8,
    /// Whether this is an ordered list
    pub is_ordered: bool,
    /// Can this list accept more items (not closed by blank line)
    pub can_continue: bool,
}

impl ParseState {
    fn new() -> Self {
        ParseState {
            list_stack: Vec::new(),
            quote_stack: Vec::new(),
            between_fences: false,
        }
    }

    /// Get the current list context if any
    fn current_list(&self) -> Option<&ListContext> {
        self.list_stack.last()
    }

    /// Get the current quote level
    fn current_quote_level(&self) -> u32 {
        self.quote_stack.len() as u32
    }

    /// Calculate the required indentation for a list item to be nested
    /// under the current list
    #[allow(dead_code)]
    fn required_nesting_indent(&self) -> u32 {
        if let Some(list) = self.current_list() {
            list.marker_column + list.marker_width
        } else {
            0
        }
    }

    /// Find the appropriate parent list for a new list item at given column
    /// Returns the depth and whether it should be nested
    #[allow(dead_code)]
    fn find_list_parent(&self, marker_column: u32, _marker_width: u32) -> (u8, bool) {
        // Check if this can be nested under any existing list
        for (_idx, list) in self.list_stack.iter().enumerate() {
            let required = list.marker_column + list.marker_width;
            if marker_column >= required {
                // Can be nested under this list
                return (list.depth + 1, true);
            }
        }
        // Not nested under any existing list
        (0, false)
    }
}

/// Parse lines
pub(crate) fn parse_lines<'s, I>(
    md_lines: I,
    options: Options,
) -> Text<'s>
where
    I: Iterator<Item = &'s str>,
{
    let mut lines = Vec::new();
    let mut state = ParseState::new();
    let mut continue_code = false;
    let mut continue_italic = false;
    let mut continue_bold = false;
    let mut continue_strikeout = false;
    
    for md_line in md_lines {
        let mut line_parser = parser::LineParser::from(md_line);
        
        // Check if line starts with quote marker
        let (is_quote, quote_indent) = count_leading_quotes(md_line);
        
        // Check if line starts with list marker and get its info
        let list_marker_info = find_list_marker(md_line);
        

        
        // Calculate depth for this list item based on parent context
        // Find the appropriate parent from the stack
        let calculated_depth = if let Some(info) = &list_marker_info {
            // Search through the stack to find the right parent
            let mut parent_depth = 0u8;
            let mut found_parent = false;
            for list_ctx in state.list_stack.iter().rev() {
                // Check if this marker is nested under this list
                if info.column >= list_ctx.marker_column + list_ctx.marker_width {
                    // Nested under this list
                    parent_depth = list_ctx.depth + 1;
                    found_parent = true;
                    break;
                } else if info.column == list_ctx.marker_column {
                    // Same column, same list
                    parent_depth = list_ctx.depth;
                    found_parent = true;
                    break;
                }
                // Continue searching up the stack
            }
            if found_parent {
                parent_depth
            } else {
                // No parent found, top-level
                0
            }
        } else {
            0
        };
        
        let line = if state.between_fences {
            continue_code = false;
            continue_italic = false;
            continue_bold = false;
            continue_strikeout = false;
            line_parser.as_code()
        } else {
            if continue_code {
                line_parser.code = true;
            }
            if continue_italic {
                line_parser.italic = true;
            }
            if continue_bold {
                line_parser.bold = true;
            }
            if continue_strikeout {
                line_parser.strikeout = true;
            }
            
            // Parse the line with context
            let line = line_parser.parse_line_with_context(
                state.current_list(),
                state.current_quote_level(),
                list_marker_info.as_ref(),
                is_quote,
                calculated_depth,
            );
            
            continue_code = options.continue_inline_code && line_parser.code;
            continue_italic = options.continue_italic && line_parser.italic;
            continue_bold = options.continue_bold && line_parser.bold;
            continue_strikeout = options.continue_strikeout && line_parser.strikeout;
            line
        };
        
        // Update state based on the parsed line
        match &line {
            Line::CodeFence(..) => {
                state.between_fences = !state.between_fences;
                if options.keep_code_fences {
                    lines.push(line);
                }
                // Close any open lists when entering/leaving code fence
                state.list_stack.clear();
            }
            Line::Normal(composite) => {
                match &composite.style {
                    CompositeStyle::Quote => {
                        // Handle blockquotes
                        if !state.quote_stack.is_empty() {
                            // Continue existing quote
                        } else {
                            // Start new quote
                            state.quote_stack.push(quote_indent);
                        }
                        lines.push(line);
                    }
                    CompositeStyle::ListItem(depth) => {
                        // Handle list items
                        if let Some(info) = list_marker_info {
                            // This line starts a new list item
                            let marker_column = info.column;
                            let marker_width = info.width;
                            let is_ordered = info.is_ordered;
                            
                            // Find or create the appropriate list context for this depth
                            // Pop contexts that are deeper than the current depth
                            while let Some(top_list) = state.current_list() {
                                if top_list.depth <= *depth {
                                    break;
                                }
                                state.list_stack.pop();
                            }
                            
                            // Check if we have a context at the current depth
                            if let Some(current_list) = state.current_list() {
                                if current_list.depth == *depth {
                                    // Same depth, reuse the same list context
                                    // Just update can_continue
                                    if let Some(top_list) = state.list_stack.last_mut() {
                                        top_list.can_continue = true;
                                    }
                                } else {
                                    // New depth, push new context
                                    state.list_stack.push(ListContext {
                                        marker_column,
                                        marker_width,
                                        depth: *depth,
                                        is_ordered,
                                        can_continue: true,
                                    });
                                }
                            } else {
                                // No active list at this depth, start new one
                                state.list_stack.push(ListContext {
                                    marker_column,
                                    marker_width,
                                    depth: *depth,
                                    is_ordered,
                                    can_continue: true,
                                });
                            }
                        }
                        lines.push(line);
                    }
                    _ => {
                        // Regular paragraph or code
                        // Check if this closes any open lists
                        // (blank lines or non-list content can close lists)
                        if md_line.trim().is_empty() {
                            // Blank line - mark lists as not continuing
                            for list in &mut state.list_stack {
                                list.can_continue = false;
                            }
                        } else if !state.list_stack.is_empty() {
                            // Non-list content - check if it's a continuation line
                            // For now, we'll handle this in the line parser
                        }
                        lines.push(line);
                    }
                }
            }
            _ => {
                lines.push(line);
            }
        }
    }
    Text { lines }
}

/// Information about a list marker found at the start of a line
#[derive(Debug, Clone)]
pub struct ListMarkerInfo {
    /// Column where the marker starts (0-indexed)
    pub column: u32,
    /// Byte offset where the marker starts (for proper string slicing)
    pub byte_offset: u32,
    /// Width of the marker including following space
    pub width: u32,
    /// Whether it's an ordered list marker
    pub is_ordered: bool,
    /// The depth this marker would create (based on CommonMark rules)
    pub depth: u8,
}

/// Find list marker at the start of a line
/// Returns None if no list marker found
fn find_list_marker(line: &str) -> Option<ListMarkerInfo> {
    // Calculate the visual column, treating tabs as 4 spaces
    let visual_column = calculate_visual_column(line);
    // Find the byte offset of the first non-whitespace character
    let byte_offset = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();
    
    if trimmed.starts_with("-") || trimmed.starts_with("*") || trimmed.starts_with("+") {
        // Bullet list marker
        let marker_len = 1;
        let after_marker = &trimmed[marker_len..];
        
        // Check if followed by space, tab, or end of line
        if after_marker.starts_with(' ') || after_marker.starts_with('\t') || after_marker.is_empty() {
            Some(ListMarkerInfo {
                column: visual_column,
                byte_offset: byte_offset as u32,
                width: marker_len as u32 + 1, // marker + space
                is_ordered: false,
                depth: 0, // Will be calculated based on context
            })
        } else {
            None
        }
    } else if let Some(digit_start) = trimmed.find(|c: char| c.is_ascii_digit()) {
        if digit_start == 0 {
            // Starts with digit - could be ordered list
            let digits_end = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
            
            if digits_end < trimmed.len() {
                let next_char = trimmed.chars().nth(digits_end).unwrap();
                if next_char == '.' || next_char == ')' {
                    let after_marker = &trimmed[digits_end + 1..];
                    // Check if followed by space, tab, or end of line
                    if after_marker.starts_with(' ') || after_marker.starts_with('\t') || after_marker.is_empty() {
                        let marker_width = digits_end as u32 + 2; // digits + marker + space
                        Some(ListMarkerInfo {
                            column: visual_column,
                            byte_offset: byte_offset as u32,
                            width: marker_width,
                            is_ordered: true,
                            depth: 0,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// Calculate the visual column position of the first non-whitespace character
/// Tabs are treated as 4 spaces each
fn calculate_visual_column(line: &str) -> u32 {
    let mut column = 0u32;
    for c in line.chars() {
        match c {
            ' ' => column += 1,
            '\t' => column += 4,
            _ => break,
        }
    }
    column
}

/// Count leading quote markers and return (is_quote, total_indent)
fn count_leading_quotes(line: &str) -> (bool, u32) {
    let trimmed = line.trim_start();
    
    if trimmed.starts_with('>') {
        let after_marker = &trimmed[1..];
        // Check if followed by space or end of line
        if after_marker.starts_with(' ') || after_marker.is_empty() {
            (true, calculate_visual_column(line))
        } else {
            (false, calculate_visual_column(line))
        }
    } else {
        (false, calculate_visual_column(line))
    }
}
