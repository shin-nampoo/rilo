extern crate termios;

use std::os::unix::io::{AsRawFd};
use std::io::{self, Write, Read, stdout, stdin,  BufRead};
use std::path::Path;
use std::fs::File;
use std::{env};
use std::time::{Instant, Duration};
use termios::*;
use terminal_size::{Width, Height, terminal_size};

pub const RILO_VERSION: u16 = 1;
pub const RILO_TAB_STOP: u16 = 8;

macro_rules! ctrl_key {
    ($ch:expr) => {
        $ch as u8 & 0x1f
    };
}

enum EditorKey{
    Arrow(Arrow),
    Function(Function),
    Else(u8),
}

enum Arrow {
    Left,
    Right,
    Up,
    Down,
}

enum Function {
    Up,
    Down,
    Home,
    End,
    Delete,
}

struct EditorConfig {
    cx: u16,
    cy: u16,
    rx: u16,
    screen_rows: u16,
    screen_cols: u16,
    termios: Termios,
    erow: Vec<Erow>,
    numrows: u16,
    rowoff: u16,
    coloff: u16,
    filename: Vec<u8>,
    statusmsg: Vec<u8>,
    statusmsg_time: Instant,
}

struct AppendBuffer {
    b: Vec<u8>,
    len: usize,
}

struct Erow {
    size: u16,
    chars: Vec<u8>,
    _rsize: u16,
    render: Vec<u8>,
}


fn ab_append(abuf: &mut AppendBuffer, s: &mut Vec<u8>) {
    abuf.b.append(s);
    abuf.len = abuf.b.len();
}

fn enable_raw_mode() -> Termios {
    let stdin = stdin().as_raw_fd();
    let mut termios = Termios::from_fd(stdin).unwrap();
    let org_termios = termios.clone();

    termios.c_iflag &= !(BRKINT | ICRNL | INPCK | ISTRIP | IXON);
    termios.c_oflag &= !(OPOST);
    termios.c_cflag |= CS8;
    termios.c_lflag &= !(ECHO | ICANON | IEXTEN | ISIG);
    termios.c_cc[VMIN] = 0;
    termios.c_cc[VTIME] = 10;
    tcsetattr(stdin, TCSAFLUSH, &termios).unwrap();    

    org_termios
}

fn disable_raw_mode(termios: Termios){
    let stdin = stdin().as_raw_fd();
    tcsetattr(stdin, TCSAFLUSH, &termios).unwrap();    
}

fn get_window_size() -> Option<(terminal_size::Width, terminal_size::Height)> {
    let size = terminal_size();
    size
}

fn editor_read_key() -> EditorKey {
    let mut c = [0u8;1];
    c[0] = b'\0';
    loop {
        match stdin().read(&mut c) { 
            Ok(0) => (),
            Ok(1) => break,
            Ok(_) => print!("hen_\r\n"),
            Err(_e) => print!("error\r\n"),
        };
    }
    if c[0] == b'\x1b' {
        let mut seq = [[0u8;1], [0u8;1], [0u8;1]];
        if let Ok(1) =  stdin().read(&mut seq[0]) {
        }else{
            return EditorKey::Else(b'\x1b')
        };
        if let Ok(1) =  stdin().read(&mut seq[1]) {
        }else{
            return EditorKey::Else(b'\x1b')
        };
        if seq[0][0] == b'[' {
            if seq[1][0] >= b'0' && seq[1][0] <= b'9' {
                if let Ok(1) =  stdin().read(&mut seq[2])  {
                    if seq[2][0] == b'~' {
                        match seq[1][0] {
                            b'1' => return EditorKey::Function(Function::Home),
                            b'3' => return EditorKey::Function(Function::Delete),
                            b'4' => return EditorKey::Function(Function::End),
                            b'5' => return EditorKey::Function(Function::Up),
                            b'6' => return EditorKey::Function(Function::Down),
                            b'7' => return EditorKey::Function(Function::Home),
                            b'8' => return EditorKey::Function(Function::End),
                            _ => return EditorKey::Else('\x1b' as u8),
                        };
                    }
                };
            }else{
                match seq[1][0] {
                    b'A' => return EditorKey::Arrow(Arrow::Up),
                    b'B' => return EditorKey::Arrow(Arrow::Down),
                    b'C' => return EditorKey::Arrow(Arrow::Right),
                    b'D' => return EditorKey::Arrow(Arrow::Left),
                    b'H' => return EditorKey::Function(Function::Home),
                    b'F' => return EditorKey::Function(Function::End),
                    _ => return EditorKey::Else(b'\x1b'),
                };
            }
        }else if seq[0][0] == b'0' {
            match seq[1][0] {
                b'H' => return EditorKey::Function(Function::Home),
                b'F' => return EditorKey::Function(Function::End),
                _ => return EditorKey::Else(b'\x1b'),
            };
        }
        return EditorKey::Else(b'\x1b')
    }else{
        EditorKey::Else(c[0])
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

fn editor_scroll(ec: &mut EditorConfig){
    ec.rx = 0;
    if ec.cy < ec.numrows {
        ec.rx = editor_row_cxtorx(ec.erow[ec.cy as usize].chars.clone(), ec.cx as usize);
    }
    
    if ec.cy < ec.rowoff {
        ec.rowoff = ec.cy;
    }
    if ec.cy >= ec.rowoff + ec.screen_rows {
        ec.rowoff = ec.cy - ec.screen_rows+ 1;
    }
    if ec.rx < ec.coloff {
        ec.coloff = ec.rx;
    }
    if ec.rx >= ec.coloff + ec.screen_cols {
        ec.coloff = ec.rx - ec.screen_cols + 1;
    }
}

fn editor_move_cursor(key: &Arrow, ec: &mut EditorConfig) {
    let mut tv_ll: u16 = 0;
    if ec.cy < ec.numrows {
        tv_ll = ec.erow[ec.cy as usize].size;
    }
    match key {
        Arrow::Left => {
            if ec.cx != 0 {
                ec.cx -= 1
            }else if ec.cy > 0 {
                ec.cy -= 1;
                ec.cx = ec.erow[ec.cy as usize].size;
            }
        },
        Arrow::Right => {
            if ec.cx < tv_ll {
                ec.cx += 1
            }else if ec.cx == tv_ll {
                ec.cy += 1;
                ec.cx = 0;
            }
        },
        Arrow::Up => {
            if ec.cy != 0 {
                ec.cy -= 1
            }
        },
        Arrow::Down => {
            if ec.cy < ec.numrows {
                ec.cy += 1
            }
        },
    }
    if ec.cy >= ec.numrows {
        ec.cx = 0;
    }else if ec.cx > ec.erow[ec.cy as usize].size {
        ec.cx = ec.erow[ec.cy as usize].size;
    }
}

fn editor_process_keypress(ec: &mut EditorConfig) -> Result<usize, & 'static str> {
    let inkey: EditorKey = editor_read_key();
    let mut inkey_val: u8 = 0;
    match inkey {
        EditorKey::Arrow(arrow) => {
            editor_move_cursor(&arrow, ec);
        },
        EditorKey::Function(func) => {
            match func {
                Function::Up | Function::Down => {
                    if let Function::Up = func {
                        ec.cy = ec.rowoff;
                    }else{
                        ec.cy = ec.rowoff + ec.screen_rows -1;
                        if ec.cy > ec.numrows {
                            ec.cy = ec.numrows;
                        }
                    }
                    let mut times = ec.screen_rows;
                    let y: Arrow = if let Function::Up = func {
                        Arrow::Up
                    } else {
                        Arrow::Down
                    };
                    while times != 0 {
                        editor_move_cursor(&y, ec);
                        times -= 1;
                    } 
                },
                Function::Home => ec.cx = 0,
                Function::End => {
                    if ec.cy < ec.numrows {
                        ec.cx = ec.erow[ec.cy as usize].size;
                    }
                },
                Function::Delete => (),
            }
        }
        EditorKey::Else(val) => {
            inkey_val = val;
        }, 
    };
    if inkey_val == ctrl_key!('q') {
        stdout().write("\x1b[2J".as_bytes()).unwrap();
        stdout().write("\x1b[H".as_bytes()).unwrap();
        return Ok(1)
    }
    Ok(0)
}

fn editor_draw_rows(w: &mut EditorConfig, abuf: &mut AppendBuffer) {
    let mut y: u16 = 0;
    while y < w.screen_rows {
        let filerow = y + w.rowoff;
        if filerow >= w.numrows {
            if w.numrows == 0 && y == w.screen_rows / 3 {
                let msg = format!("Rilo editor -- version {}", RILO_VERSION);
                let mut vmsg: Vec<u8> = msg.as_bytes().to_vec();
                let mut padding = (w.screen_cols - vmsg.len() as u16) / 2;
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
            let mut disp_vec: Vec<u8> = w.erow[filerow as usize].render.clone();
            disp_vec.truncate(disp_vec.len());
            
            if w.coloff > 0 {
                disp_vec.drain(1..w.coloff as usize);
            }

            if disp_vec.len() > w.screen_cols as usize {
                disp_vec.truncate(w.screen_cols as usize - 1);
            }
            ab_append(abuf, &mut disp_vec);
        }
        ab_append(abuf, &mut "\x1b[K".as_bytes().to_vec());
        ab_append(abuf, &mut "\r\n".as_bytes().to_vec());
        y += 1;
    }
}

fn editor_draw_status_bar(ec: &EditorConfig, abuf: &mut AppendBuffer){
    ab_append(abuf, &mut "\x1b[7m".as_bytes().to_vec());
    let mut status = ec.filename.clone();
    let mut line = format!(" - {} lines", ec.numrows); 
    status.append(&mut line.as_bytes().to_vec());
    let mut len = status.len() as u16;
    if len > ec.screen_cols {
        status.truncate(ec.screen_cols as usize);
    }
    ab_append(abuf, &mut status);
    line = format!("{}/{}", ec.cy + 1, ec.numrows);
    status.append(&mut line.as_bytes().to_vec());
    let rlen = status.len() as u16;
    while len < ec.screen_cols {
        if rlen == ec.screen_cols - len {
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

fn editor_draw_message_bar(ec: &EditorConfig, abuf: &mut AppendBuffer){
    ab_append(abuf, &mut "\x1b[K".as_bytes().to_vec());
    let mut msg = ec.statusmsg.clone();
    let mut msglen = ec.statusmsg.len() as u16;
    if msglen > ec.screen_cols {
        msglen = ec.screen_cols;
    }
    if Instant::now() - ec.statusmsg_time < Duration::from_secs(5) {
        msg.truncate(msglen as usize);
        ab_append(abuf, &mut msg);
    }
}

fn editor_refresh_screen(ec: &mut EditorConfig) {
    editor_scroll(ec);

    let mut abuf: AppendBuffer = AppendBuffer { b:Vec::<u8>::new(), len: 0, };
    ab_append(&mut abuf, &mut "\x1b[?25l".as_bytes().to_vec());
    ab_append(&mut abuf, &mut "\x1b[H".as_bytes().to_vec());

    editor_draw_rows(ec, &mut abuf);
    editor_draw_status_bar(ec, &mut abuf);
    editor_draw_message_bar(ec, &mut abuf);

    let csr = format!("\x1b[{};{}H", ec.cy - ec.rowoff + 1, ec.rx - ec.coloff + 1);
    ab_append(&mut abuf, &mut csr.as_bytes().to_vec());

    ab_append(&mut abuf, &mut "\x1b[?25h".as_bytes().to_vec());

    let pbuf = Box::into_raw(abuf.b.clone().into_boxed_slice()) as *mut u8;
    unsafe{
        let buf: &mut [u8] = core::slice::from_raw_parts_mut(pbuf, abuf.len);
        stdout().write(buf).unwrap();
    }
    stdout().flush().unwrap();
}

fn editor_set_status_message(ec: &mut EditorConfig, fmt: String) {
    ec.statusmsg = fmt.clone().as_bytes().to_vec();
}

fn editor_update_row(c_vec: Vec<u8>) -> Vec<u8> {
    let v_iter = c_vec.iter();
    let mut new_vec: Vec<u8> = Vec::new();
    for ch in v_iter {
        if *ch == b'\t' {
            let mut idx = new_vec.len();
            new_vec.push(b' ');
            idx += 1;
            while idx % RILO_TAB_STOP as usize != 0 {
                new_vec.push(b' ');
                idx += 1;
            }
        }else{
            new_vec.push(*ch);
        }
    }
    new_vec
} 

fn editor_append_row(char_vec: &mut Vec<u8>, size: u16, ec: &mut EditorConfig){
    char_vec.append(&mut "\0".as_bytes().to_vec());
    let r_vec = editor_update_row(char_vec.clone());
    let erow: Erow = Erow {
        size: size,
        chars: char_vec.clone(),
        _rsize: r_vec.len() as u16,
        render: r_vec,
    };
    ec.erow.push(erow);
    ec.numrows += 1;
}

fn editor_open(filename: &String, ec: &mut EditorConfig) {
    let path = Path::new(filename); 
    let file = File::open(path).unwrap();
    ec.filename = filename.as_bytes().to_vec();
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
                    editor_append_row(&mut vec_line, ll as u16, ec);
                }
            },
            Err(e) => panic!("File read line fail:{:?}",e),
        };
        line.clear();
    }
}

fn init_editor() -> EditorConfig {
    let mut ec: EditorConfig = EditorConfig{ 
        cx: 0, cy: 0, rx: 0,
        screen_rows: 0, screen_cols: 0,
        termios: enable_raw_mode(),
        erow: Vec::new(),
        numrows: 0,
        rowoff: 0, 
        coloff: 0,
        filename: Vec::new(),
        statusmsg: Vec::new(),
        statusmsg_time: Instant::now(),
    };
    if let Some((Width(w), Height(h))) = get_window_size() {
        ec.screen_rows = h - 2;
        ec.screen_cols = w;
    }else{
        panic!("Unable to get terminal size.");
    }
    ec
}

fn main() {
    let mut ec: EditorConfig = init_editor();
    let argc: usize = env::args().len();
    let args: Vec<String> = env::args().collect();
    if argc == 2 {
        let filename = &args[1];
        editor_open(&filename, &mut ec);
    }else{
        if argc > 2 {
            disable_raw_mode(ec.termios);
            panic!("argument not match!.");
        }
    }

    editor_set_status_message(&mut ec, String::from("HELP: Ctrl-q = quit"));

    loop {
        editor_refresh_screen(&mut ec);
        match editor_process_keypress(&mut ec) {
            Ok(0) => (),
            Ok(1) => break,
            _ => panic!("hendesu"),
        };
    }
    disable_raw_mode(ec.termios);
}
