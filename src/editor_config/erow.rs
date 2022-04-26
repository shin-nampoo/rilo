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
        let scs_len = es.singleline_comment_start.len();
        let scs = es.singleline_comment_start.clone();
        let mut idx = 0;
        let mut prev_sep = true;
        let mut prev_hl: Highlight;
        let erow_str: String = String::from_utf8(self.render.clone()).unwrap();
        while idx < self.render.len() {
            if idx > 0 {
                prev_hl = self.hl[idx - 1].clone();
            }else{
                prev_hl = Highlight::NORMAL;
            }
            if scs_len != 0 && si.in_string == 0 {
                if (self._rsize as usize) - idx >= scs_len && 
                    self.render[idx..(idx + scs_len)] == scs.as_bytes().to_vec() {
                    while idx < self.render.len() {
                        self.hl[idx] = Highlight::COMMENT;
                        idx += 1;
                    }
                    return;
                }
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
                    if self.render[idx] as char == '"' || self.render[idx] as char == '\'' {
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
                    prev_sep = false;
                    continue;
                }
            }
            if prev_sep {
                let mut kwd: String;
                let mut hlk: Highlight;
                let mut k_idx: usize = 0;
                while k_idx < es.keywords.len() {
                    kwd = es.keywords[k_idx].clone();
                    if &kwd[kwd.len() - 1..kwd.len()] == "|" {
                        kwd.pop();
                        hlk = Highlight::KEYWORD2;
                    }else{
                        hlk = Highlight::KEYWORD1;
                    }
                    if (self._rsize as usize) - idx >= kwd.len() && 
                            &erow_str[idx..(idx + kwd.len())] == &kwd[..] {
                        let hl_max = idx + kwd.len();
                        while idx < hl_max {
                            self.hl[idx] = hlk.clone();
                            idx += 1;
                        }
                        break;
                    }
                    k_idx += 1;
                }
                if k_idx != es.keywords.len(){
                    prev_sep = false;
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
