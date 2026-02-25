use std::collections::BTreeMap;
use std::time::Duration;
use zellij_tile::prelude::*;

#[derive(Default)]
struct State {
    sessions: Vec<SessionInfo>,
    resurrectable: Vec<(String, Duration)>,
    selected: usize,
    scroll_offset: usize,
    permissions_granted: bool,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ]);
        subscribe(&[
            EventType::SessionUpdate,
            EventType::Key,
            EventType::PermissionRequestResult,
        ]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                self.permissions_granted = true;
                true
            }
            Event::PermissionRequestResult(PermissionStatus::Denied) => {
                self.permissions_granted = false;
                true
            }
            Event::SessionUpdate(sessions, resurrectable) => {
                self.sessions = sessions;
                self.resurrectable = resurrectable;
                self.clamp_selection();
                true
            }
            Event::Key(key) => self.handle_key(key),
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if !self.permissions_granted {
            print_text_with_coordinates(
                Text::new("Waiting for permissions..."),
                0, 0, Some(cols), None,
            );
            return;
        }

        let total = self.session_count();
        if total == 0 {
            print_text_with_coordinates(
                Text::new("No sessions found."),
                0, 0, Some(cols), None,
            );
            return;
        }

        // Header ribbon — .selected() gives green background like built-in tabs
        print_ribbon_with_coordinates(
            Text::new(" Sessioner ").selected(),
            0, 0, Some(cols), Some(1),
        );

        // Footer: keybindings as plain text (no ribbon = no colored background)
        let footer_y = rows.saturating_sub(1);
        let footer = "\u{2191}\u{2193}/jk navigate  Enter attach  d kill dead  D kill all dead  Esc quit";
        print_text_with_coordinates(
            Text::new(footer)
                .color_substring(3, "\u{2191}\u{2193}/jk")
                .color_substring(3, "Enter")
                .color_substring(3, "d")
                .color_nth_substring(3, "D", 1)
                .color_substring(3, "Esc"),
            0, footer_y, Some(cols), Some(1),
        );

        // Body: nested list of sessions + pane titles
        let body_height = rows.saturating_sub(2);
        if body_height == 0 {
            return;
        }

        let items = self.build_list_items(body_height);
        print_nested_list_with_coordinates(items, 0, 1, Some(cols), Some(body_height));
    }
}

impl State {
    fn session_count(&self) -> usize {
        self.sessions.len() + self.resurrectable.len()
    }

    fn clamp_selection(&mut self) {
        let total = self.session_count();
        if total == 0 {
            self.selected = 0;
        } else if self.selected >= total {
            self.selected = total - 1;
        }
    }

    /// Collect pane titles for a session, excluding plugin panes.
    fn pane_titles(session: &SessionInfo) -> Vec<String> {
        let mut titles = Vec::new();
        let mut tab_indices: Vec<&usize> = session.panes.panes.keys().collect();
        tab_indices.sort();
        for tab_idx in tab_indices {
            if let Some(panes) = session.panes.panes.get(tab_idx) {
                for pane in panes {
                    if pane.is_plugin {
                        continue;
                    }
                    titles.push(pane.title.clone());
                }
            }
        }
        titles
    }

    /// Build the nested list items with scroll support.
    fn build_list_items(&mut self, visible_rows: usize) -> Vec<NestedListItem> {
        struct SessionBlock {
            header_line: usize,
            pane_titles: Vec<String>,
            is_current: bool,
            is_resurrectable: bool,
            connected_clients: usize,
            session_idx: usize,
        }

        let mut blocks = Vec::new();
        let mut line = 0usize;

        for (i, session) in self.sessions.iter().enumerate() {
            let titles = Self::pane_titles(session);
            let block_size = 1 + titles.len();
            blocks.push(SessionBlock {
                header_line: line,
                pane_titles: titles,
                is_current: session.is_current_session,
                is_resurrectable: false,
                connected_clients: session.connected_clients,
                session_idx: i,
            });
            line += block_size;
        }

        for (i, (_name, age)) in self.resurrectable.iter().enumerate() {
            blocks.push(SessionBlock {
                header_line: line,
                pane_titles: vec![format!("exited {}", format_duration(*age))],
                is_current: false,
                is_resurrectable: true,
                connected_clients: 0,
                session_idx: self.sessions.len() + i,
            });
            line += 2;
        }

        let total_lines = line;

        // Scroll to keep selected session visible
        let selected_block = blocks.iter().find(|b| b.session_idx == self.selected);
        let selected_header = selected_block.map(|b| b.header_line).unwrap_or(0);
        let selected_size = selected_block
            .map(|b| 1 + b.pane_titles.len())
            .unwrap_or(1);

        if selected_header < self.scroll_offset {
            self.scroll_offset = selected_header;
        } else if selected_header + selected_size > self.scroll_offset + visible_rows {
            self.scroll_offset =
                (selected_header + selected_size).saturating_sub(visible_rows);
        }
        if total_lines <= visible_rows {
            self.scroll_offset = 0;
        } else if self.scroll_offset > total_lines.saturating_sub(visible_rows) {
            self.scroll_offset = total_lines.saturating_sub(visible_rows);
        }

        // Emit visible NestedListItems
        let mut items = Vec::new();
        let mut current_line = 0usize;
        let visible_end = self.scroll_offset + visible_rows;

        for block in &blocks {
            let is_selected = block.session_idx == self.selected;

            let name = if block.is_resurrectable {
                self.resurrectable
                    .get(block.session_idx - self.sessions.len())
                    .map(|(n, _)| n.as_str())
                    .unwrap_or("?")
            } else {
                self.sessions
                    .get(block.session_idx)
                    .map(|s| s.name.as_str())
                    .unwrap_or("?")
            };

            // Session header
            if current_line >= self.scroll_offset && current_line < visible_end {
                let suffix = if block.is_current {
                    " (attached)"
                } else if block.is_resurrectable {
                    " (exited)"
                } else if block.connected_clients > 0 {
                    " (connected)"
                } else {
                    ""
                };

                let header_text = format!("{}{}", name, suffix);
                let name_len = name.len();
                // Color 0 (orange) for session name, color 2 (green) for status
                let mut item =
                    NestedListItem::new(&header_text).color_range(0, ..name_len);
                if !suffix.is_empty() {
                    item = item.color_range(2, name_len..header_text.len());
                }
                if is_selected {
                    item = item.selected();
                }
                items.push(item);
            }
            current_line += 1;

            // Pane titles
            for title in &block.pane_titles {
                if current_line >= self.scroll_offset && current_line < visible_end {
                    let mut item =
                        NestedListItem::new(title).indent(1).color_range(1, ..);
                    if is_selected {
                        item = item.selected();
                    }
                    items.push(item);
                }
                current_line += 1;
            }
        }

        items
    }

    fn handle_key(&mut self, key: KeyWithModifier) -> bool {
        let total = self.session_count();
        if total == 0 {
            if key.is_key_without_modifier(BareKey::Char('q'))
                || key.is_key_without_modifier(BareKey::Esc)
            {
                close_self();
            }
            return false;
        }

        if key.is_key_without_modifier(BareKey::Up)
            || key.is_key_without_modifier(BareKey::Char('k'))
        {
            if self.selected > 0 {
                self.selected -= 1;
            }
            return true;
        }

        if key.is_key_without_modifier(BareKey::Down)
            || key.is_key_without_modifier(BareKey::Char('j'))
        {
            if self.selected < total - 1 {
                self.selected += 1;
            }
            return true;
        }

        if key.is_key_without_modifier(BareKey::Enter) {
            self.switch_to_selected();
            return false;
        }

        if key.is_key_without_modifier(BareKey::Char('d')) {
            self.delete_selected_dead();
            return true;
        }

        if key.is_key_without_modifier(BareKey::Char('D')) {
            delete_all_dead_sessions();
            return true;
        }

        if key.is_key_without_modifier(BareKey::Char('q'))
            || key.is_key_without_modifier(BareKey::Esc)
        {
            close_self();
            return false;
        }

        false
    }

    fn switch_to_selected(&self) {
        if self.selected < self.sessions.len() {
            let session = &self.sessions[self.selected];
            if session.is_current_session {
                close_self();
                return;
            }
            switch_session(Some(&session.name));
        } else {
            let idx = self.selected - self.sessions.len();
            if let Some((name, _)) = self.resurrectable.get(idx) {
                switch_session(Some(name));
            }
        }
    }

    fn delete_selected_dead(&self) {
        if self.selected >= self.sessions.len() {
            let idx = self.selected - self.sessions.len();
            if let Some((name, _)) = self.resurrectable.get(idx) {
                delete_dead_session(name);
            }
        }
    }
}

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}
