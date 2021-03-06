use crate::App;
use crate::util::Role;

use chrono::Timelike;
use tui::backend::Backend;
use tui::layout::{Constraint, Corner, Direction, Layout, Rect};
use tui::style::{Color, Style};
use tui::text::{Span, Spans, Text};
use tui::widgets::{Block, Borders, List, ListItem, Paragraph};
use tui::Frame;
use unicode_width::UnicodeWidthStr;

pub fn draw<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let has_members = app
        .data
        .channels
        .state
        .selected()
        .and_then(|idx| app.data.channels.items.get(idx))
        .map(|channel| !channel.members.is_empty())
        .unwrap_or(false);

    let chunks = match has_members {
        false => Layout::default()
                    .constraints([Constraint::Ratio(1, 4), Constraint::Ratio(3, 4)].as_ref())
                    .direction(Direction::Horizontal)
                    .split(f.size()),
        true => Layout::default()
                    .constraints([Constraint::Ratio(1, 4), Constraint::Ratio(5, 8), Constraint::Ratio(1, 8)].as_ref())
                    .direction(Direction::Horizontal)
                    .split(f.size())
    };

    let channel_list_width = chunks[0].width.saturating_sub(2) as usize;
    let channels: Vec<ListItem> = app
        .data
        .channels
        .items
        .iter()
        .map(|channel| {
            let unread_messages_label = if channel.unread_messages != 0 {
                format!(" ({})", channel.unread_messages)
            } else {
                String::new()
            };
            let channel_name = channel.bestname();
            let label = format!("{}{}", channel_name, unread_messages_label);
            let label_width = label.width();
            let label = if label.width() <= channel_list_width || unread_messages_label.is_empty() {
                label
            } else {
                let diff = label_width - channel_list_width;
                let mut end = channel_name.width().saturating_sub(diff);
                while !channel_name.is_char_boundary(end) {
                    end += 1;
                }
                format!("{}{}", &channel_name[0..end], unread_messages_label)
            };
            ListItem::new(vec![Spans::from(Span::raw(label))])
        })
        .collect();
    let channels = List::new(channels)
        .block(Block::default().borders(Borders::ALL).title("Channels"))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Gray));
    f.render_stateful_widget(channels, chunks[0], &mut app.data.channels.state);

    draw_chat(f, app, chunks[1]);
    if has_members {
        draw_members(f, app, chunks[2]);
    }
}

fn draw_chat<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
    let text_width = area.width.saturating_sub(2) as usize;
    let lines = app
        .data
        .input
        .chars()
        .enumerate()
        .fold(Vec::new(), |mut lines, (idx, c)| {
            if idx % text_width == 0 {
                lines.push(String::new())
            }
            lines.last_mut().unwrap().push(c);
            lines
        });
    let num_input_lines = lines.len().max(1);
    let input: Vec<Spans> = lines.into_iter().map(Spans::from).collect();
    let extra_cursor_line = if app.data.input_cursor > 0 && app.data.input_cursor % text_width == 0
    {
        1
    } else {
        0
    };

    let chunks = Layout::default()
        .constraints(
            [
                Constraint::Min(0),
                Constraint::Length(num_input_lines as u16 + 2 + extra_cursor_line),
            ]
            .as_ref(),
        )
        .direction(Direction::Vertical)
        .split(area);

    draw_messages(f, app, chunks[0]);

    let input = Paragraph::new(Text::from(input))
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input, chunks[1]);
    f.set_cursor(
        // Put cursor past the end of the input text
        chunks[1].x + ((app.data.input_cursor as u16) % text_width as u16) + 1,
        // Move one line down, from the border to the input line
        chunks[1].y + (app.data.input_cursor as u16 / (text_width as u16)) + 1,
    );
}

fn draw_messages<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    let messages = app
        .data
        .channels
        .state
        .selected()
        .and_then(|idx| app.data.channels.items.get(idx))
        .map(|channel| &channel.messages[..])
        .unwrap_or(&[]);

    let max_username_width = messages
        .iter()
        .map(|msg| displayed_name(&msg.from, true).width())
        .max()
        .unwrap_or(0);

    let description = app
        .data
        .channels
        .state
        .selected()
        .and_then(|idx| app.data.channels.items.get(idx))
        .map(|channel| &*channel.description)
        .unwrap_or("Messages");
    let room_description = match description {
        "" => "Messages",
        d => d,
    };

    let width = area.width - 2; // without borders
    let max_lines = area.height;

    let time_style = Style::default().fg(Color::Yellow);
    let messages = messages
        .iter()
        .rev()
        // we can't show more messages atm and don't have messages navigation
        .take(max_lines as usize)
        .map(|msg| {
            let arrived_at = msg.arrived_at.with_timezone(&chrono::Local);

            let time = Span::styled(
                format!("{:02}:{:02} ", arrived_at.hour(), arrived_at.minute()),
                time_style,
            );
            let from = displayed_name(&msg.from, true);
            let from = Span::styled(
                textwrap::indent(&from, &" ".repeat(max_username_width - from.width())),
                Style::default().fg(user_color(&msg.from)),
            );
            let delimeter = Span::from(": ");

            let prefix_width = (time.width() + from.width() + delimeter.width()) as u16;
            let indent = " ".repeat(prefix_width.into());
            let message = msg.message.clone();
            let lines =
                textwrap::wrap_iter(message.as_str(), width.saturating_sub(prefix_width).into());

            let spans: Vec<Spans> = lines
                .enumerate()
                .map(|(idx, line)| {
                    let res = if idx == 0 {
                        vec![
                            time.clone(),
                            from.clone(),
                            delimeter.clone(),
                            Span::from(line.to_string()),
                        ]
                    } else {
                        vec![Span::from(format!("{}{}", indent, line))]
                    };
                    Spans::from(res)
                })
                .collect();
            spans
        });

    let mut items: Vec<_> = messages.map(|s| ListItem::new(Text::from(s))).collect();

    if let Some(selected_idx) = app.data.channels.state.selected() {
        let unread_messages = app.data.channels.items[selected_idx].unread_messages;
        if unread_messages > 0 && unread_messages < items.len() {
            let prefix_width = max_username_width + 8;
            let new_message_line = "-".repeat(prefix_width)
                + "new messages"
                + &"-".repeat((width as usize).saturating_sub(prefix_width));

            items.insert(unread_messages, ListItem::new(Span::from(new_message_line)));
        }
    }

    let list = List::new(items)
        .block(
            Block::default()
                .title(room_description)
                .borders(Borders::ALL),
        )
        .style(Style::default().fg(Color::White))
        .start_corner(Corner::BottomLeft);
    f.render_widget(list, area);
}

fn draw_members<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    let members = app
        .data
        .channels
        .state
        .selected()
        .and_then(|idx| app.data.channels.items.get(idx))
        .map(|channel| &channel.members[..])
        .unwrap_or(&[]);

    let max_lines = area.height;

    let present_style = Style::default().fg(Color::White);
    let absent_style = Style::default().fg(Color::Red);
    let members = members
        .iter()
        .rev()
        // we can't show more members atm and don't have members navigation
        .take(max_lines as usize)
        .map(|member| {
            let present = app.data.tracked_presences.get(&member.hash);
            let style = match present {
                Some(true) => present_style,
                _ => absent_style,
            };
            let role = match member.role {
                Role::Admin => String::from("👑"),
                Role::Member => String::from("-"),
                Role::Invited => String::from("⏳"),
            };

            let name = app.data.profile_manager.display_name(&member.hash);
            let uri = Span::styled(
                format!("{} {}", role, name),
                style,
            );

            uri
        });

    let items: Vec<_> = members.map(|s| ListItem::new(Text::from(s))).collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL),
        )
        .style(Style::default().fg(Color::White))
        .start_corner(Corner::TopLeft);
    f.render_widget(list, area);
}

// Randomly but deterministically choose a color for a username
fn user_color(username: &str) -> Color {
    use Color::*;
    const COLORS: &[Color] = &[Red, Green, Yellow, Blue, Magenta, Cyan, Gray];
    let idx = username
        .bytes()
        .map(|b| usize::from(b) % COLORS.len())
        .sum::<usize>()
        % COLORS.len();
    COLORS[idx]
}

fn displayed_name(name: &str, first_name_only: bool) -> &str {
    if first_name_only {
        let space_pos = name.find(' ').unwrap_or_else(|| name.len());
        &name[0..space_pos]
    } else {
        &name
    }
}
