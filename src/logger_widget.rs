//! # Logger with smart widget for the `tui` and `ratatui` crate

use std::sync::Mutex;

use chrono::{DateTime, Local};
use ratatui::{
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::List,
};

pub use self::circular::CircularBuffer;

use std::sync::OnceLock;

static TUI_LOGGER: OnceLock<TuiLogger> = OnceLock::new();

pub fn init_tui_logger(buffer_size: usize) {
    TUI_LOGGER.get_or_init(|| TuiLogger {
        records: Mutex::new(CircularBuffer::new(buffer_size)),
    });
}

pub fn tui_tracing_subscriber() -> TuiTracingSubscriber {
    TuiTracingSubscriber
}

struct Record {
    timestamp: DateTime<Local>,
    level: tracing::Level,
    #[allow(dead_code)]
    file: String,
    #[allow(dead_code)]
    line: u32,
    target: String,
    msg: String,
}

pub struct TuiLogger {
    records: Mutex<CircularBuffer<Record>>,
}

pub struct TuiTracingSubscriber;

// Implement tracing layer
impl<S> tracing_subscriber::Layer<S> for TuiTracingSubscriber
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = tracing_visitor::ToStringVisitor::default();
        event.record(&mut visitor);

        let record = Record {
            timestamp: chrono::Local::now(),
            level: *event.metadata().level(),
            file: event.metadata().file().unwrap_or("?").to_string(),
            line: event.metadata().line().unwrap_or(0),
            target: event.metadata().target().to_string(),
            msg: format!("{}", visitor),
        };

        TUI_LOGGER
            .get()
            .unwrap()
            .records
            .lock()
            .unwrap()
            .push(record);
    }
}

pub struct LogWidget {
    format_timestamp: String,
}

impl Default for LogWidget {
    fn default() -> LogWidget {
        LogWidget {
            format_timestamp: "%H:%M:%S".to_string(),
        }
    }
}

impl LogWidget {
    pub fn draw<'a>(self, area_width: usize) -> List<'a> {
        // Raw string lines
        let mut lines: Vec<Text> = vec![];

        // Get the records lock
        let mut records = TUI_LOGGER.get().unwrap().records.lock().unwrap();

        // Loop records
        for record in records.iter() {
            let message = record.msg.lines().next_back().unwrap().to_string();

            let level_style = match record.level {
                tracing::Level::INFO => Style::default().fg(Color::Green),
                tracing::Level::WARN => Style::default().fg(Color::Yellow),
                tracing::Level::DEBUG => Style::default().fg(Color::Magenta),
                tracing::Level::TRACE => Style::default().fg(Color::Cyan),
                tracing::Level::ERROR => Style::default().fg(Color::Red),
            };

            let timestamp = format!("{} ", record.timestamp.format(&self.format_timestamp));
            let level = format!("{} ", record.level);
            let target = format!("{}: ", record.target);

            // if record.level == tracing::Level::ERROR {
            //     let location = format!("{}:{}", record.file, record.line);
            //     output_line.push(' ');
            //     output_line.push_str(&location);
            // }

            let line_len = level.len() + target.len() + timestamp.len();

            let timestamp = Span::styled(timestamp, Style::default().fg(Color::Gray));
            let level = Span::styled(level, level_style);
            let target = Span::styled(target, Style::default().fg(Color::Gray));

            let first_part =
                textwrap::wrap(&message, textwrap::Options::new(area_width - line_len))
                    .first()
                    .unwrap()
                    .to_string();

            let first_part = Span::styled(first_part, Style::default());

            let line = Line::from(vec![timestamp, level, target, first_part]);

            let rest = textwrap::wrap(&message, textwrap::Options::new(area_width - line_len))
                .iter()
                .skip(1)
                .map(|s| Line::from(s.to_string()))
                .collect::<Vec<_>>();

            let mut log_line = vec![];

            log_line.push(line);
            log_line.extend(rest);

            let text = Text::from(log_line);
            lines.push(text);
        }

        // Drop the records lock
        drop(records);
        List::new(lines).scroll_padding(2)
    }
}

#[rustfmt::skip]
pub mod tracing_visitor {
    use std::{fmt, error, collections::HashMap};
    use tracing::field::Visit;

    #[derive(Default)]
    pub struct ToStringVisitor<'a>(HashMap<&'a str, String>);

    impl fmt::Display for ToStringVisitor<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.iter().try_for_each(|(_k, v)| -> fmt::Result { write!(f, "{}", v) })
        }
    }
    
    impl<'a> Visit for ToStringVisitor<'a> {
        fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
            self.0.insert(field.name(), format_args!("{}", value).to_string());
        }

        fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
            self.0.insert(field.name(), format_args!("{}", value).to_string());
        }

        fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
            self.0.insert(field.name(), format_args!("{}", value).to_string());
        }

        fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
            self.0.insert(field.name(), format_args!("{}", value).to_string());
        }

        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            self.0.insert(field.name(), format_args!("{}", value).to_string());
        }

        fn record_error(&mut self, field: &tracing::field::Field, value: &(dyn error::Error + 'static)) {
            self.0.insert(field.name(), format_args!("{}", value).to_string());
        }

        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
            self.0.insert(field.name(), format_args!("{:?}", value).to_string());
        }
    }
}

#[rustfmt::skip]
mod circular {
    use std::{iter, slice};
    
    pub struct CircularBuffer<T> {
        buffer: Vec<T>,
        next_write_pos: usize,
    }
    
    impl<T> CircularBuffer<T> {
        /// Create a new `CircularBuffer`, which can hold `max_depth` elements
        pub fn new(max_depth: usize) -> Self {
            Self {
                buffer: Vec::with_capacity(max_depth),
                next_write_pos: 0,
            }
        }    
        /// Elements are pushed until the capacity is reached.
        /// Afterwards the oldest elements will be overwritten.
        pub fn push(&mut self, elem: T) {
            let buffer_capacity = self.buffer.capacity();

            match self.buffer.len() < buffer_capacity {
                true => self.buffer.push(elem),
                false => self.buffer[self.next_write_pos % buffer_capacity] = elem,
            }
        
            self.next_write_pos += 1;
        }
        /// Take out all elements from the buffer, leaving an empty buffer behind
        pub fn take(&mut self) -> Vec<T> {
            let mut consumed = vec![];
        
            match self.buffer.len() < self.buffer.capacity() {
                true => consumed.append(&mut self.buffer),
                false => { 
                    let wrap_idx = self.next_write_pos % self.buffer.capacity();
                    consumed.append(&mut self.buffer.split_off(wrap_idx));
                    consumed.append(&mut self.buffer);
                }
            }
        
            self.next_write_pos = 0;
            consumed
        }
        /// Return an iterator to step through all elements in the sequence,
        /// as these have been pushed (FIFO)
        pub fn iter(&mut self) -> iter::Chain<slice::Iter<T>, slice::Iter<T>> {
            // Check if buffer is completely filled
            match self.next_write_pos < self.buffer.capacity() {
                // If not, then just iterate through it
                true => self.buffer[0..].iter().chain(self.buffer[..0].iter()),
                // If yes, find wrap around index and chain around it
                false => {
                    let wrap_idx = self.next_write_pos % self.buffer.capacity();
                    self.buffer[wrap_idx..].iter().chain(self.buffer[..wrap_idx].iter())
                }
            }
        }
        /// Return an iterator to step through all elements in the reverse sequence,
        /// as these have been pushed (LIFO)
        pub fn rev_iter(&mut self) -> iter::Chain<iter::Rev<slice::Iter<T>>, iter::Rev<slice::Iter<T>>> {
            // Check if buffer is completely filled
            match self.next_write_pos < self.buffer.capacity() {
                // If not, then just reverse iterate through it
                true => self.buffer[0..].iter().rev().chain(self.buffer[..0].iter().rev()),
                // If yes, find wrap around index and chain around it
                false => {
                    let wrap_idx = self.next_write_pos % self.buffer.capacity();
                    self.buffer[..wrap_idx].iter().rev().chain(self.buffer[wrap_idx..].iter().rev())
                }
            }
        }
    }
}
