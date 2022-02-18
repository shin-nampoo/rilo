mod erow;
pub use crate::editor_config::erow::{Erow};
use super::{EditorKey, ab_append, editor_read_key, AppendBuffer, Function, Arrow, 
            enable_raw_mode, get_window_size};

use std::io::{self, Write, stdout, BufRead};
use std::path::Path;
use std::fs::{File};
use std::{str};
use std::time::{Instant, Duration};
use termios::*;
use terminal_size::{Width, Height};
use bitflags::bitflags;


pub const RILO_VERSION: u16 = 1;
pub const RILO_TAB_STOP: u16 = 8;
pub const RILO_QUIT_TIMES: u16 = 3;

macro_rules! ctrl_key {
    ($ch:expr) => {
        $ch as u8 & 0x1f
    };
}

bitflags! {
    pub struct HLFlags: u32 {
        const HLF_NUMBERS = 0b00000001;
        const HLF_STRINGS = 0b00000010;
    }
}

#[derive(Clone)]
pub struct EditorSyntaxInf {
    syntax: Option<EditorSyntax>,
    in_string: u8,
}

#[derive(Clone)]
pub struct EditorSyntax {
    file_type: String,
    file_extensions: Vec<String>,
    pub flags: HLFlags,
    pub numbers: u8,
}

impl EditorSyntax {
    fn new(f_type: &str, f_ext: Vec<String>, hf: HLFlags) -> EditorSyntax {
        EditorSyntax{
            file_type: String::from(f_type),
            file_extensions: f_ext.clone(),
            flags: hf,
            numbers: 0,
        }
    }

    fn much_type(&mut self, filename: &Vec<u8>) -> bool {
        let str_fname = String::from_utf8(filename.clone()).unwrap();
        let work_vec: Vec<&str> = str_fname.split('.').collect();
        if work_vec.len() == 1 { return false };
        let ext: &str = work_vec[work_vec.len() - 1];
        for syntax_ext in self.file_extensions.iter(){
            if syntax_ext == ext {
                return true
            }
        }
        return false
    }
}

#[derive(Clone)]
pub enum Highlight {
    NONE,
    NORMAL,
    NUMBER,
    MATCH,
    STRING,
}

struct CurrentPosition {x: u16, y: u16}
struct Screen { rows: u16, cols: u16}
struct Offset {row: u16, col: u16}
struct Status { message: Vec<u8>, time: Instant}

pub struct EditorConfig {
    cp: CurrentPosition,
    rx: u16,
    screen: Screen,
    pub termios: Termios,
    erow: Vec<Erow>,
    numrows: u16,
    off: Offset,
    filename: Vec<u8>,
    status: Status,
    dirty: bool,
    quit_times: u16,
    last_match: i32,
    direction: i32,
    current_color: Highlight,
    saved_hl_line: i16,
    saved_hl: Vec<Highlight>,
    pub editor_syntax: EditorSyntaxInf,
    syntax_pattern: Vec<EditorSyntax>,
}

impl EditorConfig {

    fn editor_move_cursor(&mut self, key: &Arrow) {
        let mut tv_ll: u16 = 0;
        if self.cp.y < self.numrows {
            tv_ll = self.erow[self.cp.y as usize].size;
        }
        match key {
            Arrow::Left => {
                if self.cp.x != 0 {
                    self.cp.x -= 1
                }else if self.cp.y > 0 {
                    self.cp.y -= 1;
                    self.cp.x = self.erow[self.cp.y as usize].size;
                }
            },
            Arrow::Right => {
                if self.cp.x < tv_ll {
                    self.cp.x += 1
                }else if self.cp.x == tv_ll {
                    self.cp.y += 1;
                    self.cp.x = 0;
                }
            },
            Arrow::Up => {
                if self.cp.y != 0 {
                    self.cp.y -= 1
                }
            },
            Arrow::Down => {
                if self.cp.y < self.numrows {
                    self.cp.y += 1
                }
            },
        }
        if self.cp.y >= self.numrows {
            self.cp.x = 0;
        }else if self.cp.x > self.erow[self.cp.y as usize].size {
            self.cp.x = self.erow[self.cp.y as usize].size;
        }
    }
    
    pub fn editor_process_keypress(&mut self) -> Result<usize, & 'static str> {
        let inkey: EditorKey = editor_read_key();
        match inkey {
            EditorKey::Arrow(arrow) => {
                self.editor_move_cursor(&arrow);
            },
            EditorKey::Function(func) => {
                match func {
                    Function::Up | Function::Down => {
                        if let Function::Up = func {
                            self.cp.y = self.off.row;
                        }else{
                            self.cp.y = self.off.row + self.screen.rows -1;
                            if self.cp.y > self.numrows {
                                self.cp.y = self.numrows;
                            }
                        }
                        let mut times = self.screen.rows;
                        let y: Arrow = if let Function::Up = func {
                            Arrow::Up
                        } else {
                            Arrow::Down
                        };
                        while times != 0 {
                            self.editor_move_cursor(&y);
                            times -= 1;
                        } 
                    },
                    Function::Home => self.cp.x = 0,
                    Function::End => {
                        if self.cp.y < self.numrows {
                            self.cp.x = self.erow[self.cp.y as usize].size;
                        }
                    },
                    Function::Delete => {
                        self.editor_move_cursor(&Arrow::Right);
                        self.editor_delete_char();
                    },
                    Function::Backspace => {
                        self.editor_delete_char();
                    },
                }
            }
            EditorKey::Else(val) => {
                if val == ctrl_key!('q') {
                    if self.dirty && self.quit_times > 0 {
                        self.editor_set_status_message(
                            format!(
                            "WARNING!!! File has unsaves changes. Press Ctrl-Q {} more times to quit.",
                            self.quit_times)
                        );
                        self.quit_times -= 1;
                        return Ok(0)
                    }
                    stdout().write("\x1b[2J".as_bytes()).unwrap();
                    stdout().write("\x1b[H".as_bytes()).unwrap();
                    return Ok(1)
                }else if val == ctrl_key!('h') {
                    self.editor_delete_char();
                }else if val == ctrl_key!('f') {
                    self.editor_find();
                }else if val == ctrl_key!('s') {
                    self.editor_save();
                }else if val == '\r' as u8 {
                    self.editor_insert_new_line();
                }else if val == '\x1b' as u8 {
                }else{
                    self.editor_insert_char(&val);
                }
            }, 
        };
        Ok(0)
    }
    
    pub fn editor_select_syntax_highlight(&mut self){
        self.editor_syntax.syntax = None;
        if self.filename.len() == 0 {
            return;
        }
        let pattern = self.syntax_pattern.clone();
        for syntax in pattern.iter(){
            let mut ts = syntax.clone();
            if ts.much_type(&self.filename) {
                self.editor_syntax.syntax = Some(ts.clone());
                let mut filerow: usize = 0;
                while filerow < self.numrows as usize {
                    self.erow[filerow].editor_update_syntax(&mut self.editor_syntax);
                    filerow += 1;
                }
            }
        }
    }

    pub fn editor_open(&mut self, filename: &String) {
        let path = Path::new(filename); 
        let file = File::open(path).unwrap();
        self.filename = filename.as_bytes().to_vec();
        self.editor_select_syntax_highlight();
        let mut reader = io::BufReader::new(file);
        let mut line = String::new();
        loop{
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(len) => {
                    let mut vec_line: Vec<u8> = line.clone().as_bytes().to_vec();
                    let mut ll = len;
                    while ll > 0 && (vec_line[ll - 1] == '\r' as u8 || vec_line[ll - 1] == '\n' as u8) {
                        ll -= 1;
                    }
                    if ll != 0 {
                        vec_line.truncate(ll);
                        let at = self.numrows;
                        self.editor_insert_row(&at, &mut vec_line, ll as u16);
                    }
                },
                Err(e) => panic!("File read line fail:{:?}",e),
            };
            line.clear();
        }
    }

    fn editor_insert_row(&mut self, at: &u16, char_vec: &mut Vec<u8>, size: u16){
        if *at > self.numrows {
            return;
        }
        char_vec.append(&mut "\0".as_bytes().to_vec());
        let mut erow: Erow = Erow {
            size: size,
            chars: char_vec.clone(),
            _rsize: 0,
            render: Vec::new(),
            hl: Vec::new(),
        };
        erow.editor_update_row(&mut self.editor_syntax);
        self.erow.insert(*at as usize, erow);
        self.numrows += 1;
    }
    
    pub fn editor_scroll(&mut self){
        self.rx = 0;
        if self.cp.y < self.numrows {
            self.rx = editor_row_cxtorx(self.erow[self.cp.y as usize].chars.clone(), self.cp.x as usize);
        }
        
        if self.cp.y < self.off.row {
            self.off.row = self.cp.y;
        }
        if self.cp.y >= self.off.row + self.screen.rows {
            self.off.row = self.cp.y - self.screen.rows + 1;
        }
        if self.rx < self.off.col {
            self.off.col = self.rx;
        }
        if self.rx >= self.off.col + self.screen.cols {
            self.off.col = self.rx - self.screen.cols + 1;
        }
    }

    pub fn editor_set_status_message(&mut self, fmt: String) {
        self.status.message = fmt.clone().as_bytes().to_vec();
        self.status.time = Instant::now();
    }

    pub fn editor_prompt(&mut self, prompt: String,
            fb: Option<fn(&mut EditorConfig, &String, &EditorKey)>) -> Vec<u8> {
        let mut buf: String = String::new();
        loop{
            let mut message = String::new();
            for c in prompt.as_str().chars(){
                if c == '{' {
                    for buf_c in buf.chars(){
                        message.push(buf_c);
                    }
                }else if c == '}' {
                }else {
                    message.push(c);
                }
            }
            self.editor_set_status_message(message);
            self.editor_refresh_screen();

            let keyin = editor_read_key();
            match keyin {
                EditorKey::Function(Function::Backspace) |
                EditorKey::Function(Function::Delete) => {
                    buf.pop();
                },
                EditorKey::Else(val) => {
                    if val == '\r' as u8 {
                        if buf.len() != 0 {
                            self.editor_set_status_message(String::from(""));
                            if let Some(call_back) = fb {
                                call_back(self, &buf, &keyin);
                            } 
                            return buf.as_bytes().to_vec();
                        }
                    }else if val == '\x1b' as u8 {
                        self.editor_set_status_message(String::from(""));
                        if let Some(call_back) = fb {
                            call_back(self, &buf, &keyin);
                        } 
                        return "".as_bytes().to_vec();
                    }else if val != ctrl_key!('c') && val < 128 {
                        buf.push(val as char);
                    }else if val == ctrl_key!('h') as u8 {
                        buf.pop();
                    }
                },
                _ => (),
            }
            if let Some(call_back) = fb {
                call_back(self, &buf, &keyin);
            } 
        }
    }

    pub fn editor_refresh_screen(&mut self) {
        self.editor_scroll();
    
        let mut abuf: AppendBuffer = AppendBuffer { b:Vec::<u8>::new(), len: 0, };
        ab_append(&mut abuf, &mut "\x1b[?25l".as_bytes().to_vec());
        ab_append(&mut abuf, &mut "\x1b[H".as_bytes().to_vec());
    
        self.editor_draw_rows(&mut abuf);
        self.editor_draw_status_bar(&mut abuf);
        self.editor_draw_message_bar(&mut abuf);
    
        let csr = format!("\x1b[{};{}H", self.cp.y - self.off.row + 1, self.rx - self.off.col + 1);
        ab_append(&mut abuf, &mut csr.as_bytes().to_vec());
    
        ab_append(&mut abuf, &mut "\x1b[?25h".as_bytes().to_vec());
    
        let pbuf = Box::into_raw(abuf.b.clone().into_boxed_slice()) as *mut u8;
        unsafe{
            let buf: &mut [u8] = core::slice::from_raw_parts_mut(pbuf, abuf.len);
            stdout().write(buf).unwrap();
        }
        stdout().flush().unwrap();
    }
    
    fn editor_draw_rows(&mut self, abuf: &mut AppendBuffer) {
        let mut y: u16 = 0;
        while y < self.screen.rows {
            let filerow = y + self.off.row;
            if filerow >= self.numrows {
                if self.numrows == 0 && y == self.screen.rows / 3 {
                    let msg = format!("Rilo editor -- version {}", RILO_VERSION);
                    let mut vmsg: Vec<u8> = msg.as_bytes().to_vec();
                    let mut padding = (self.screen.cols - vmsg.len() as u16) / 2;
                    if padding != 0 {
                        ab_append(abuf, &mut "~".as_bytes().to_vec());
                        padding  -= 1;
                    }
                    while padding != 0 {
                        ab_append(abuf, &mut " ".as_bytes().to_vec());
                        padding -= 1;
                    }
                    ab_append(abuf, &mut vmsg);
                }else{
                    ab_append(abuf, &mut "~".as_bytes().to_vec());
                }
            }else{
                let mut disp_vec: Vec<u8> = self.erow[filerow as usize].render.clone();
                disp_vec.truncate(disp_vec.len());
                
                if self.off.col > 0 {
                    disp_vec.drain(1..self.off.col as usize);
                }
    
                if disp_vec.len() > self.screen.cols as usize {
                    disp_vec.truncate(self.screen.cols as usize - 1);
                }
                let hl_vec: Vec<Highlight> = self.erow[filerow as usize].hl.clone();
                let mut idx = 0;
                while idx < hl_vec.len() {
                    if let Highlight::NORMAL = hl_vec[idx]  {
                        if !matches!(self.current_color, Highlight::NORMAL) {
                            ab_append(abuf, &mut "\x1b[39m".as_bytes().to_vec());
                            self.current_color = Highlight::NORMAL;
                        }
                        ab_append(abuf, &mut std::slice::from_ref(&disp_vec[idx]).to_vec());
                    }else{
                        let color: u8 = editor_syntax_to_color(&hl_vec[idx]);
                        if color != editor_syntax_to_color(&self.current_color) {
                            self.current_color = editor_color_to_syntax(color);
                            let clen = format!("\x1b[{}m", color); 
                            ab_append(abuf, &mut clen.as_bytes().to_vec());
                        }
                        ab_append(abuf, &mut std::slice::from_ref(&disp_vec[idx]).to_vec());
                    }
                    idx += 1;
                }
                ab_append(abuf, &mut "\x1b[39m".as_bytes().to_vec());
            }
            ab_append(abuf, &mut "\x1b[K".as_bytes().to_vec());
            ab_append(abuf, &mut "\r\n".as_bytes().to_vec());
            y += 1;
        }

    }

    fn editor_draw_message_bar(&self, abuf: &mut AppendBuffer){
        ab_append(abuf, &mut "\x1b[K".as_bytes().to_vec());
        let mut msg = self.status.message.clone();
        let mut msglen = self.status.message.len() as u16;
        if msglen > self.screen.cols {
            msglen = self.screen.cols;
        }
        if Instant::now() - self.status.time < Duration::from_secs(5) {
            msg.truncate(msglen as usize);
            ab_append(abuf, &mut msg);
        }
    }

    fn editor_draw_status_bar(&mut self, abuf: &mut AppendBuffer){
        ab_append(abuf, &mut "\x1b[7m".as_bytes().to_vec());
        let mut status = self.filename.clone();
        let mut line = format!(" - {} lines", self.numrows); 
        status.append(&mut line.as_bytes().to_vec());
        if self.dirty {
            status.append(&mut "(modified)".as_bytes().to_vec());
        }
        let mut len = status.len() as u16;
        if len > self.screen.cols {
            status.truncate(self.screen.cols as usize);
        }
        ab_append(abuf, &mut status);
        let st = self.editor_syntax.syntax.clone();
        let ft: String = match st {
            Some(val) => val.file_type,
            None => String::from("no ft")
        };
        line = format!("{} | {}/{}", ft, self.cp.y + 1, self.numrows);
        status.append(&mut line.as_bytes().to_vec());
        let rlen = status.len() as u16;
        while len < self.screen.cols {
            if rlen == self.screen.cols - len {
                ab_append(abuf, &mut status);
                break;
            }else{
                ab_append(abuf, &mut " ".as_bytes().to_vec());
            }
            len += 1;
        }
        ab_append(abuf, &mut "\x1b[m".as_bytes().to_vec());
        ab_append(abuf, &mut "\r\n".as_bytes().to_vec());
    }
    
    fn editor_insert_new_line(&mut self){
        let mut at = self.cp.y;
        if self.cp.x == 0 {
            self.editor_insert_row(&at, &mut "".as_bytes().to_vec(), 0)
        }else{
            at += 1;
            let mut row = self.erow[self.cp.y as usize].chars.split_off(self.cp.x as usize);
            self.erow[self.cp.y as usize].size =
                (self.erow[self.cp.y as usize].chars.len() - 1) as u16;
            self.erow[self.cp.y as usize].editor_update_row(&mut self.editor_syntax);
            let size = row.len() as u16;
            self.editor_insert_row(&at, &mut row,  size)
        }
        self.cp.y += 1;
        self.cp.x = 0;
        self.dirty = true;
    }
    
    fn editor_insert_char(&mut self, c: &u8){
        if self.cp.y == self.numrows {
            let at = self.cp.y;
            self.editor_insert_row(&at, &mut "".as_bytes().to_vec(), 0);
        }
        let mut at: i16 = self.cp.x as i16; 
        self.erow[self.cp.y as usize].editor_row_insert_character(&mut at, *c, &mut self.editor_syntax);
        self.cp.x += 1;
        self.dirty = true;
    }
    
    fn editor_delete_char(&mut self){
        if self.cp.y == self.numrows {
            return;
        }
        let mut at: i16 = (self.cp.x - 1) as i16;
        self.erow[self.cp.y as usize].editor_row_delete_char(&mut at, &mut self.editor_syntax);
        self.cp.x -= 1;
        self.dirty = true;
    }
    
    fn editor_rows_to_string(&mut self) -> Vec<u8> {
        let mut buf_vec: Vec<u8> = Vec::new();
        for v_erow in self.erow.iter_mut() {
            let mut temp_vec = v_erow.chars.clone();
            temp_vec.pop();
            temp_vec.push('\n' as u8);
            buf_vec.append(&mut temp_vec);
        }
        buf_vec
    }
    
    fn editor_save(&mut self) {
        if self.filename.is_empty() {
            let cb: Option<fn(&mut EditorConfig, &String, &EditorKey)> = None;
            self.filename = self.editor_prompt(String::from("Save as: {} (ESC to cancel)"), cb);
            if self.filename.len() == 0 { 
                self.editor_set_status_message("Save aborted".to_string());
                return;
            }
            self.editor_select_syntax_highlight();
        }
        let path = String::from_utf8(self.filename.clone()).unwrap();
        let w_vec: Vec<u8> = self.editor_rows_to_string();
        let len = w_vec.len();
        std::fs::write(path, w_vec).unwrap();
        self.editor_set_status_message(format!("{} bytes written to disk", len));
        self.dirty = false;
        self.quit_times = RILO_QUIT_TIMES;
    }
    
    fn editor_find(&mut self){
        let saved_cx = self.cp.x;
        let saved_cy = self.cp.y;
        let saved_coloff = self.off.col;
        let saved_rowoff = self.off.row;
    
        let cb: Option<fn(&mut EditorConfig, &String, &EditorKey)> = Some(editor_find_callback);
        let query = String::from_utf8(
            self.editor_prompt(String::from("Search: {} (Use ESC/Arrows/Enter)"), cb)).unwrap();
    
        if query.len() == 0 {
            self.cp.x = saved_cx;
            self.cp.y = saved_cy;
            self.off.col = saved_coloff;
            self.off.row = saved_rowoff;
        }
    }
    
    pub fn new() -> EditorConfig {
        let mut ec: EditorConfig = EditorConfig{
            cp: CurrentPosition{x: 0, y: 0}, 
            rx: 0,
            screen: Screen{ rows: 0, cols: 0},
            termios: enable_raw_mode(),
            erow: Vec::new(),
            numrows: 0,
            off: Offset{ row: 0 ,col: 0},
            filename: Vec::new(),
            status: Status {message: Vec::new(), time: Instant::now()},
            dirty: false,
            quit_times: RILO_QUIT_TIMES,
            last_match: -1,
            direction: 1,
            current_color: Highlight::NONE,
            saved_hl: Vec::new(),
            saved_hl_line: -1,
            editor_syntax: EditorSyntaxInf {syntax:None, in_string:0},
            syntax_pattern: vec![
                EditorSyntax::new("rust", 
                    vec![String::from("rs"), String::from("toml")], 
                    HLFlags::HLF_NUMBERS | HLFlags::HLF_STRINGS),
            ],
        };
        if let Some((Width(w), Height(h))) = get_window_size() {
            ec.screen.rows = h - 2;
            ec.screen.cols = w;
        }else{
            panic!("Unable to get terminal size.");
        }
        ec
    }
        
}


fn editor_row_cxtorx(vec: Vec<u8>, cx: usize) -> u16 {
    let mut rx: u16 = 0;
    let t_vec: Vec<u8> = vec[0..cx].to_vec();
    let v_iter = t_vec.iter();
    for ch in v_iter {
        if *ch == b'\t' {
            rx += (RILO_TAB_STOP - 1) - ( rx % RILO_TAB_STOP) + 1;
        }else{
            rx += 1;
        }
    }
    rx
}

fn editor_row_rxtocx(vec: Vec<u8>, rx: usize) -> u16 {
    let mut cx: u16 = 0;
    let mut cur_rx: u16 = 0;
    let v_iter = vec.iter();
    for ch in v_iter {
        if *ch == b'\t' {
            cur_rx += (RILO_TAB_STOP - 1) - ( cur_rx % RILO_TAB_STOP) + 1;
        }else{
            cur_rx += 1;
        }
        if cur_rx > rx as u16 {
            return cx;
        }
        cx += 1;
    }
    cx
}

fn editor_syntax_to_color(hl: &Highlight) -> u8 {
    match hl {
        Highlight::NUMBER => 31,
        Highlight::MATCH => 34,
        Highlight::STRING => 35,
        _ => 37
    }
}

fn editor_color_to_syntax(color: u8) -> Highlight {
    if color == 31 {
        Highlight::NUMBER
    }else if color == 34 {
        Highlight::MATCH
    }else if color == 35 {
        Highlight::STRING
    }else {
        Highlight::NORMAL
    }
}

fn editor_find_callback(ec: &mut EditorConfig, query: &String, key: &EditorKey) {
    if ec.saved_hl.len() != 0 {
        ec.erow[ec.saved_hl_line as usize].hl = ec.saved_hl.clone();
        ec.saved_hl.clear();
    }
    match key {
        EditorKey::Arrow(Arrow::Right) | EditorKey::Arrow(Arrow::Down) => ec.direction = 1,
        EditorKey::Arrow(Arrow::Left) | EditorKey::Arrow(Arrow::Up) => ec.direction = -1,
        EditorKey::Else(val) => {
            if *val == '\r' as u8 || *val == '\x1b' as u8 {
                ec.last_match = -1;
                ec.direction = 1;
                return;
            }else{
                ec.last_match = -1;
                ec.direction = 1;
            }
        },
        _ => {
            ec.last_match = -1;
            ec.direction = 1;
        },
    }

    let mut i: usize = 0;
    let q_len = query.len();
    if ec.last_match == -1 {
        ec.direction = 1;
    }
    let mut current = ec.last_match;
    while i < ec.numrows as usize {
        current += ec.direction;
        if current == -1 {
            current = (ec.numrows - 1) as i32;
        }else if current == ec.numrows as i32 {
            current = 0;
        }
        let erow: String = String::from_utf8(ec.erow[current as usize].render.clone()).unwrap();
        if q_len <= erow.len() {
            let mut pt: usize = 0;
            while pt <= erow.len() - q_len {
                if query == &erow[pt..(pt + q_len)]{
                    ec.last_match = current;
                    ec.cp.y = current as u16;
                    ec.cp.x = editor_row_rxtocx(ec.erow[current as usize].chars.clone(), pt);
                    ec.off.row = ec.numrows;
                    ec.saved_hl_line = current as i16;
                    ec.saved_hl = ec.erow[current as usize].hl.clone();
                    let mut idx = 0;
                    while idx < q_len {
                        ec.erow[current as usize].hl[pt + idx] = Highlight::MATCH;
                        idx += 1;
                    }
                    return;
                }
                pt += 1;
            }
        }
        i += 1;
    }
}
