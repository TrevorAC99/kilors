use std::{
    fs::File,
    io::{stdout, BufRead, BufReader, Write},
    process::exit,
    usize,
};

use crossterm::{cursor::{Hide, MoveTo, Show}, event::{read, Event, KeyCode, KeyEvent}, execute, terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode, size}};

const TAB_STOP_LENGTH: u16 = 8;

struct EditorRow {
    text_raw: String,
    text_render: Vec<char>,
}

impl EditorRow {
    fn from(str: String) -> Self {
        let mut row = Self {
            text_raw: str,
            text_render: Vec::new(),
        };
        row.update();
        row
    }

    fn update(&mut self) {
        self.text_render = Vec::new();
        let mut index = 0;
        for char in self.text_raw.chars() {
            match char {
                '\t' => {
                    self.text_render.push(' ');
                    index += 1;
                    let tab_width = TAB_STOP_LENGTH - (index % TAB_STOP_LENGTH);
                    for i in 0..tab_width {
                        self.text_render.push(' ');
                    }
                }
                char => {
                    self.text_render.push(char);
                    index += 1;
                }
            }
        }
    }
}

struct EditorState {
    cursor_row: u16,
    cursor_col: u16,
    row_offset: u16,
    col_offset: u16,
    screen_rows: u16,
    screen_cols: u16,
    rows: Vec<EditorRow>,
    file_name: String,
}

impl EditorState {
    fn init() -> crossterm::Result<Self> {
        let (columns, rows) = size()?;
        Ok(Self {
            cursor_row: 0,
            cursor_col: 0,
            row_offset: 0,
            col_offset: 0,
            screen_rows: rows,
            screen_cols: columns,
            rows: Vec::new(),
            file_name: String::new(),
        })
    }

    fn move_cursor(&mut self, direction: Direction) {
        let row = self.rows.get(self.cursor_row as usize);

        match direction {
            Direction::Left => {
                if self.cursor_col != 0 {
                    self.cursor_col -= 1;
                } else if self.cursor_row > 0 {
                    self.cursor_row -= 1;
                    self.cursor_col = self.rows[self.cursor_row as usize].text_render.len() as u16;
                }
            }
            Direction::Right => {
                if let Some(row) = row {
                    if (self.cursor_col as usize) < row.text_render.len() {
                        self.cursor_col += 1;
                    } else if (self.cursor_col as usize) == row.text_render.len() {
                        self.cursor_row += 1;
                        self.cursor_col = 0;
                    }
                }
            }
            Direction::Up => {
                if self.cursor_row != 0 {
                    self.cursor_row -= 1;
                }
            }
            Direction::Down => {
                if (self.cursor_row as usize) < self.rows.len() {
                    self.cursor_row += 1;
                }
            }
        }

        let row = self.rows.get(self.cursor_row as usize);
        let row_length = row.map_or(0, |row| row.text_render.len()) as u16;
        if self.cursor_col > row_length {
            self.cursor_col = row_length;
        }
    }

    fn handle_keypress(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left => self.move_cursor(Direction::Left),
            KeyCode::Right => self.move_cursor(Direction::Right),
            KeyCode::Up => self.move_cursor(Direction::Up),
            KeyCode::Down => self.move_cursor(Direction::Down),
            KeyCode::Esc => {
                cleanup();
                exit(0);
            }
            _ => {}
        }
    }

    fn load_file(&mut self, path: &str) -> std::io::Result<()> {
        let file = File::open(path)?;
        let lines = BufReader::new(file).lines();

        for line in lines {
            let line = line?;
            let row = EditorRow::from(line);
            self.rows.push(row);
        }

        Ok(())
    }

    fn scroll(&mut self) {
        if self.cursor_row < self.row_offset {
            self.row_offset = self.cursor_row
        }
        if self.cursor_row >= self.row_offset + self.screen_rows {
            self.row_offset = self.cursor_row - self.screen_rows + 1;
        }

        if self.cursor_col < self.col_offset {
            self.col_offset = self.cursor_col;
        }
        if self.cursor_col >= self.col_offset + self.screen_cols {
            self.col_offset = self.cursor_col - self.screen_cols + 1;
        }
    }

    fn draw_rows(&self) -> crossterm::Result<()> {
        for row_num in 0..self.screen_rows {
            let file_row = row_num + self.row_offset;

            let row_text = if file_row as usize >= self.rows.len() {
                String::from("~")
            } else {
                let text_render = &self.rows[file_row as usize].text_render;
                
                if text_render.len() > self.col_offset as usize {
                    let mut len = text_render.len() - self.col_offset as usize;
                    len = if len > self.screen_cols as usize {
                        self.screen_cols as usize
                    } else {
                        len
                    };
                    let slice_range = self.col_offset as usize..self.col_offset as usize + len;
                    text_render[slice_range].into_iter().collect()
                } else {
                    String::new()
                }
            };
            execute!(stdout(), Clear(ClearType::CurrentLine))?;
            stdout().write(row_text.as_bytes())?;
            if row_num < self.screen_rows - 1{
                stdout().write("\r\n".as_bytes())?;
            }
        }

        stdout().flush()?;

        Ok(())
    }

    fn refresh_screen(&mut self) -> crossterm::Result<()> {
        self.scroll();

        execute!(stdout(), Hide, MoveTo(0, 0))?;

        self.draw_rows()?;

        execute!(stdout(), MoveTo(self.cursor_col - self.col_offset, self.cursor_row - self.row_offset), Show)?;

        Ok(())
    }
}

enum Direction {
    Up,
    Down,
    Left,
    Right,
}

fn event_loop(state: &mut EditorState) -> crossterm::Result<()> {
    loop {
        state.refresh_screen()?;
        let event = read()?;

        match event {
            Event::Resize(columns, rows) => {
                // I have no idea why these plus 1s are need but they are
                state.screen_cols = columns + 1;
                state.screen_rows = rows + 1;
            }
            Event::Key(key) => {
                state.handle_keypress(key);
            }
            Event::Mouse(_) => {}
        }
    }
}

fn setup() -> crossterm::Result<()> {
    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    Ok(())
}

fn cleanup() -> crossterm::Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn run() -> crossterm::Result<()> {
    setup()?;

    let mut state = EditorState::init()?;
    state.load_file("./src/main.rs")?;

    event_loop(&mut state)?;

    cleanup()
}

fn main() {
    if let Err(e) = run() {
        println!("Error: {:?}\r", e);
    }
}
