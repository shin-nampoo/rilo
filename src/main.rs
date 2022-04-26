extern crate termios;
mod editor_config;
pub use crate::editor_config::{EditorConfig};

use std::os::unix::io::{AsRawFd};
use std::io::{Read, stdin};
use std::{env};
use termios::*;
use terminal_size::{terminal_size};
// comment test
pub enum EditorKey{
    Arrow(Arrow),  //comment test2
    Function(Function),
    Else(u8),
}

pub enum Arrow {
    Left,
    Right,
    Up,
    Down,
}

pub enum Function {
    Up,
    Down,
    Home,
    End,
    Delete,
    Backspace,
}

struct AppendBuffer {
    b: Vec<u8>,
    len: usize,
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
    termios.c_cc[VTIME] = 1;
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
    }else if c[0] == 127 {
        return EditorKey::Function(Function::Backspace)
    }else{
        EditorKey::Else(c[0])
    }
}

fn main() {
    let mut ec: EditorConfig = EditorConfig::new();
    let argc: usize = env::args().len();
    let args: Vec<String> = env::args().collect();
    if argc == 2 {
        let filename = &args[1];
        ec.editor_open(&filename);
    }else{
        if argc > 2 {
            disable_raw_mode(ec.termios);
            panic!("argument not match!.");
        }
    }

    ec.editor_set_status_message(
        String::from("HELP: Ctrl-s = save | Ctrl-q = quit | Ctrl-f = find"));

    loop {
        ec.editor_refresh_screen();
        match ec.editor_process_keypress() {
            Ok(0) => (),
            Ok(1) => break,
            _ => panic!("hendesu"),
        };
    }
    disable_raw_mode(ec.termios);
}
