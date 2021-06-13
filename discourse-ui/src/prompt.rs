use std::convert::TryFrom;

use crate::{
    backend::Backend,
    error, events,
    layout::Layout,
    style::{Color, Stylize},
    Widget,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Delimiter {
    Parentheses,
    Braces,
    SquareBracket,
    AngleBracket,
    Other(char, char),
    None,
}

impl From<Delimiter> for Option<(char, char)> {
    fn from(delim: Delimiter) -> Self {
        match delim {
            Delimiter::Parentheses => Some(('(', ')')),
            Delimiter::Braces => Some(('{', '}')),
            Delimiter::SquareBracket => Some(('[', ']')),
            Delimiter::AngleBracket => Some(('<', '>')),
            Delimiter::Other(start, end) => Some((start, end)),
            Delimiter::None => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Prompt<M, H = &'static str> {
    message: M,
    hint: Option<H>,
    delim: Delimiter,
    message_len: u16,
    hint_len: u16,
}

impl<M: AsRef<str>, H: AsRef<str>> Prompt<M, H> {
    pub fn new(message: M) -> Self {
        Self {
            message_len: u16::try_from(message.as_ref().chars().count())
                .expect("message must fit within a u16"),
            message,
            hint: None,
            delim: Delimiter::Parentheses,
            hint_len: 0,
        }
    }

    pub fn with_hint(mut self, hint: H) -> Self {
        self.hint_len = u16::try_from(hint.as_ref().chars().count())
            .expect("hint must fit within a u16");
        self.hint = Some(hint);
        self
    }

    pub fn with_optional_hint(self, hint: Option<H>) -> Self {
        match hint {
            Some(hint) => self.with_hint(hint),
            None => self,
        }
    }

    pub fn with_delim(mut self, delim: Delimiter) -> Self {
        self.delim = delim;
        self
    }

    pub fn message(&self) -> &M {
        &self.message
    }

    pub fn hint(&self) -> Option<&H> {
        self.hint.as_ref()
    }

    pub fn delim(&self) -> Delimiter {
        self.delim
    }

    pub fn into_message(self) -> M {
        self.message
    }

    pub fn into_hint(self) -> Option<H> {
        self.hint
    }

    pub fn into_message_and_hint(self) -> (M, Option<H>) {
        (self.message, self.hint)
    }

    pub fn message_len(&self) -> u16 {
        self.message_len
    }

    pub fn hint_len(&self) -> u16 {
        if self.hint.is_some() {
            match self.delim {
                Delimiter::None => self.hint_len,
                _ => self.hint_len + 2,
            }
        } else {
            0
        }
    }

    pub fn width(&self) -> u16 {
        if self.hint.is_some() {
            // `? <message> <hint> `
            2 + self.message_len + 1 + self.hint_len() + 1
        } else {
            // `? <message> › `
            2 + self.message_len + 3
        }
    }

    fn cursor_pos_impl(&self, layout: Layout) -> (u16, u16) {
        let mut width = self.width();
        if width > layout.line_width() {
            width -= layout.line_width();

            (width % layout.width, 1 + width / layout.width)
        } else {
            (layout.line_offset + width, 0)
        }
    }

    pub fn line_offset(&self, layout: Layout) -> u16 {
        self.cursor_pos_impl(layout).0
    }
}

impl<M: AsRef<str>> Prompt<M, &'static str> {
    /// `✔ <message> · `
    pub fn write_finished_message<B: Backend>(
        message: &M,
        backend: &mut B,
    ) -> error::Result<()> {
        backend.write_styled(&crate::symbols::TICK.light_green())?;
        backend.write_all(b" ")?;
        backend.write_styled(&message.as_ref().bold())?;
        backend.write_all(b" ")?;
        backend.write_styled(&crate::symbols::MIDDLE_DOT.dark_grey())?;
        backend.write_all(b" ")?;
        Ok(())
    }
}

impl<M: AsRef<str>, H: AsRef<str>> Widget for Prompt<M, H> {
    fn render<B: Backend>(
        &mut self,
        layout: &mut Layout,
        b: &mut B,
    ) -> error::Result<()> {
        b.write_styled(&"? ".light_green())?;
        b.write_styled(&self.message.as_ref().bold())?;
        b.write_all(b" ")?;

        b.set_fg(Color::DarkGrey)?;

        match (&self.hint, self.delim.into()) {
            (Some(hint), Some((start, end))) => {
                write!(b, "{}{}{}", start, hint.as_ref(), end)?
            }
            (Some(hint), None) => write!(b, "{}", hint.as_ref())?,
            (None, _) => {
                write!(b, "{}", crate::symbols::SMALL_ARROW)?;
            }
        }

        b.set_fg(Color::Reset)?;
        b.write_all(b" ")?;

        *layout = layout.with_cursor_pos(self.cursor_pos_impl(*layout));

        Ok(())
    }

    fn height(&mut self, layout: &mut Layout) -> u16 {
        let cursor_pos = self.cursor_pos_impl(*layout);
        *layout = layout.with_cursor_pos(cursor_pos);
        cursor_pos.1 + 1
    }

    fn cursor_pos(&mut self, layout: Layout) -> (u16, u16) {
        self.cursor_pos_impl(layout)
    }

    fn handle_key(&mut self, _: events::KeyEvent) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        backend::{TestBackend, TestBackendOp::*},
        style::Attributes,
        test_consts::*,
    };

    use super::*;

    type Prompt = super::Prompt<&'static str, &'static str>;

    #[test]
    fn test_width() {
        assert_eq!(Prompt::new("Hello").width(), 10);
        assert_eq!(Prompt::new("Hello").with_hint("world").width(), 16);
        assert_eq!(
            Prompt::new("Hello")
                .with_hint("world")
                .with_delim(Delimiter::None)
                .width(),
            14
        );
        assert_eq!(Prompt::new(LOREM).with_hint(UNICODE).width(), 946);
    }

    #[test]
    fn test_render() {
        fn test(
            message: &'static str,
            hint: Option<&'static str>,
            delim: Delimiter,
            delim_chars: Option<(char, char)>,
            expected_layout: Layout,
        ) {
            let mut ops = Vec::with_capacity(10);

            ops.push(SetFg(Color::LightGreen));
            ops.push(Write("? ".into()));
            ops.push(SetFg(Color::Reset));
            ops.push(SetAttributes(Attributes::BOLD));
            ops.push(Write(message.into()));
            ops.push(SetAttributes(Attributes::RESET));
            ops.push(Write(" ".into()));
            ops.push(SetFg(Color::DarkGrey));
            if let Some(hint) = hint {
                match delim_chars {
                    Some((start, end)) => {
                        ops.push(Write(start.to_string().into()));
                        ops.push(Write(hint.into()));
                        ops.push(Write(end.to_string().into()));
                    }
                    None => ops.push(Write(hint.into())),
                }
            } else {
                ops.push(Write(
                    crate::symbols::SMALL_ARROW.to_string().into_bytes(),
                ));
            }
            ops.push(SetFg(Color::Reset));
            ops.push(Write(" ".into()));

            let size = (100, 100).into();
            let mut layout = Layout::new(5, size);
            let mut prompt = Prompt::new(message)
                .with_optional_hint(hint)
                .with_delim(delim);

            prompt
                .render(&mut layout, &mut TestBackend::new(ops, size))
                .unwrap();

            assert_eq!(
                layout,
                expected_layout,
                "\ncursor pos = {:?}, width = {:?}",
                prompt.cursor_pos(Layout::new(5, size)),
                prompt.width(),
            );
        }

        let layout = Layout::new(5, (100, 100).into());

        test(
            "Hello",
            None,
            Delimiter::None,
            None,
            layout.with_line_offset(15),
        );

        test(
            "Hello",
            Some("world"),
            Delimiter::Parentheses,
            Some(('(', ')')),
            layout.with_line_offset(21),
        );

        test(
            "Hello",
            Some("world"),
            Delimiter::Braces,
            Some(('{', '}')),
            layout.with_line_offset(21),
        );

        test(
            "Hello",
            Some("world"),
            Delimiter::SquareBracket,
            Some(('[', ']')),
            layout.with_line_offset(21),
        );

        test(
            "Hello",
            Some("world"),
            Delimiter::AngleBracket,
            Some(('<', '>')),
            layout.with_line_offset(21),
        );

        test(
            "Hello",
            Some("world"),
            Delimiter::Other('-', '|'),
            Some(('-', '|')),
            layout.with_line_offset(21),
        );

        test(
            LOREM,
            Some(UNICODE),
            Delimiter::None,
            None,
            layout.with_line_offset(49).with_offset(0, 9),
        );
    }

    #[test]
    fn test_height() {
        let mut layout = Layout::new(5, (100, 100).into());

        assert_eq!(Prompt::new("Hello").height(&mut layout.clone()), 1);
        assert_eq!(
            Prompt::new("Hello")
                .with_hint("world")
                .height(&mut layout.clone()),
            1
        );
        assert_eq!(
            Prompt::new(LOREM).with_hint(UNICODE).height(&mut layout),
            10
        );
    }

    #[test]
    fn test_cursor_pos() {
        let layout = Layout::new(5, (100, 100).into());

        assert_eq!(Prompt::new("Hello").cursor_pos_impl(layout), (15, 0));
        assert_eq!(
            Prompt::new("Hello")
                .with_hint("world")
                .cursor_pos_impl(layout),
            (21, 0)
        );
        assert_eq!(
            Prompt::new("Hello")
                .with_hint("world")
                .with_delim(Delimiter::None)
                .cursor_pos_impl(layout),
            (19, 0)
        );
        assert_eq!(
            Prompt::new(LOREM)
                .with_hint(UNICODE)
                .cursor_pos_impl(layout),
            (51, 9)
        );
    }
}
