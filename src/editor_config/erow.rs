use super::{Highlight, EditorSyntax, EditorSyntaxInf, HLFlags};

pub const RILO_TAB_STOP: u16 = 8;

pub struct Erow {
    pub size: u16,
    pub chars: Vec<u8>,
    pub _rsize: u16,
    pub render: Vec<u8>,
    pub hl: Vec<Highlight>,
}

impl Erow {
    pub fn editor_update_syntax(&mut self, si: &mut EditorSyntaxInf){
        self.hl.clear();
        self.hl = vec![Highlight::NORMAL; self.render.len()];
        let es: &EditorSyntax;
        match &si.syntax {
            None => {
                return;
            },
            Some(val) => es = val,
        }
        let mut idx = 0;
        let mut prev_sep = false;
        let mut prev_hl = Highlight::NORMAL;
        while idx < self.render.len() {
            if idx > 0 {
                prev_hl = self.hl[idx - 1].clone();
            }
            if es.flags.contains(HLFlags::HLF_STRINGS) {
                if si.in_string != 0 {
                    self.hl[idx] = Highlight::STRING;
                    if self.render[idx] as char == '\\' && idx + 1 < self._rsize as usize {
                        self.hl[idx + 1] = Highlight::STRING;
                        idx += 2;
                        continue;
                    }
                    if self.render[idx] == si.in_string { si.in_string = 0; }
                    idx += 1;
                    prev_sep = true;
                    continue;
                }else{
                    if self.render[idx] as char == '"' || self.render[idx] as char == '/' {
                        si.in_string = self.render[idx];
                        self.hl[idx] = Highlight::STRING;
                        idx += 1;
                        continue;
                    }
                }
            }
            if es.flags.contains(HLFlags::HLF_NUMBERS) {
                if (self.render[idx] as char).is_numeric() &&
                        ( matches!(prev_hl, Highlight::NUMBER) || prev_sep){
                    self.hl[idx] = Highlight::NUMBER;
                    idx += 1;
                    prev_sep = true;
                    continue;
                }
            }
            prev_sep = is_separator(self.render[idx] as char);
            idx += 1;
        }
    }

    pub fn editor_row_insert_character(&mut self, at: &mut i16, c: u8, si: &mut EditorSyntaxInf){
        if *at < 0 || *at > self.size as i16 {
            *at = self.size as i16;
        }
        self.chars.insert(*at as usize, c);
        self.size += 1;
        self.editor_update_row(si);
    }

    pub fn editor_row_delete_char(&mut self, at: &mut i16, si: &mut EditorSyntaxInf){
        self.chars.remove(*at as usize);
        self.size -= 1;
        self.editor_update_row(si);
    }

    pub fn editor_update_row(&mut self, si: &mut EditorSyntaxInf) {
        let temp_vec = self.chars.clone();
        let v_iter = temp_vec.iter();
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
        self.render = new_vec;
        self._rsize = (self.render.len() - 1) as u16;
        self.editor_update_syntax(si);
    } 

}


fn is_separator(c: char) -> bool {
    match c {
        ' ' | ',' | '.' | '(' | ')' | '+' | '-' | '/' | '*' |
        '=' | '~' | '%' | '<' | '>' | '[' | ']' | ';' => true,
        _ => false,
    }
}
